#[cfg(feature = "db-auth")]
use crate::db_auth::{Role, User};
use crate::{
    column, common, table::SchemaContent, types::SqlType, ColumnDef, ColumnName, DataError,
    Database, DatabaseName, DbError, FromDao, TableDef, TableName, ToValue, Value,
};
use r2d2::ManageConnection;
use r2d2_mysql::{
    self,
    mysql::{self, prelude::Queryable},
};
use rustorm_dao::{FromDao, Rows};
use thiserror::Error;

pub fn init_pool(
    db_url: &str,
) -> Result<r2d2::Pool<r2d2_mysql::MysqlConnectionManager>, MysqlError> {
    test_connection(db_url)?;
    let opts = mysql::Opts::from_url(db_url)?;
    let builder = mysql::OptsBuilder::from_opts(opts);
    let manager = r2d2_mysql::MysqlConnectionManager::new(builder);
    let pool = r2d2::Pool::new(manager)?;
    Ok(pool)
}

pub fn test_connection(db_url: &str) -> Result<(), MysqlError> {
    let opts = mysql::Opts::from_url(db_url)?;
    let builder = mysql::OptsBuilder::from_opts(opts);
    let manager = r2d2_mysql::MysqlConnectionManager::new(builder);
    let mut conn = manager.connect()?;
    manager.is_valid(&mut conn)?;
    Ok(())
}

pub struct MysqlDB(pub r2d2::PooledConnection<r2d2_mysql::MysqlConnectionManager>);

impl Database for MysqlDB {
    fn begin_transaction(&mut self) -> Result<(), DbError> {
        self.execute_sql_with_return("START TRANSACTION", &[])?;
        Ok(())
    }

    fn commit_transaction(&mut self) -> Result<(), DbError> {
        self.execute_sql_with_return("COMMIT TRANSACTION", &[])?;
        Ok(())
    }

    fn rollback_transaction(&mut self) -> Result<(), DbError> {
        self.execute_sql_with_return("ROLLBACK TRANSACTION", &[])?;
        Ok(())
    }

    fn execute_sql_with_return(&mut self, sql: &str, param: &[&Value]) -> Result<Rows, DbError> {
        fn collect(rows: Vec<mysql::Row>) -> Result<Rows, DbError> {
            let columns = rows.first().into_iter().flat_map(mysql::Row::columns_ref);
            let column_types: Vec<_> = columns.clone().map(|c| c.column_type()).collect();
            let column_names = columns
                .map(|c| std::str::from_utf8(c.name_ref()).map(ToString::to_string))
                .collect::<Result<Vec<String>, _>>()
                .map_err(|e| MysqlError::Utf8Error(e))?;

            let mut records = Rows::new(column_names);
            for row in rows {
                records.push(into_record(row, &column_types)?);
            }

            Ok(records)
        }

        if param.is_empty() {
            let rows = self
                .0
                .query(&sql)
                .map_err(|e| MysqlError::SqlError(e, sql.to_string()))?;

            collect(rows)
        } else {
            let stmt = self
                .0
                .prep(&sql)
                .map_err(|e| MysqlError::SqlError(e, sql.to_string()))?;

            let params: mysql::Params = param
                .iter()
                .map(|v| MyValue(v))
                .map(|v| mysql::prelude::ToValue::to_value(&v))
                .collect::<Vec<_>>()
                .into();

            let rows = self
                .0
                .exec(stmt, &params)
                .map_err(|e| MysqlError::SqlError(e, sql.to_string()))?;

            collect(rows)
        }
    }

    fn get_table(&mut self, table_name: &TableName) -> Result<Option<TableDef>, DbError> {
        #[derive(Debug, FromDao)]
        struct TableSpec {
            schema: String,
            name: String,
            comment: String,
            is_view: i32,
        }

        let schema = table_name
            .schema
            .as_ref()
            .map(String::as_str)
            .unwrap_or("__DUMMY__")
            .into();
        let table_name = &table_name.name.clone().into();

        let mut tables: Vec<TableSpec> = self
            .execute_sql_with_return(
                r#"
                SELECT TABLE_SCHEMA AS `schema`,
                       TABLE_NAME AS name,
                       TABLE_COMMENT AS comment,
                       CASE TABLE_TYPE WHEN 'VIEW' THEN TRUE ELSE FALSE END AS is_view
                  FROM INFORMATION_SCHEMA.TABLES
                 WHERE TABLE_SCHEMA = CASE ? WHEN '__DUMMY__' THEN DATABASE() ELSE ? END AND TABLE_NAME = ?"#,
                &[
                    &schema, &schema,
                    table_name,
                ],
            )?
            .iter()
            .map(|dao| FromDao::from_dao(&dao))
            .collect();

        let table_spec = match tables.len() {
            0 => return Err(DbError::DataError(DataError::ZeroRecordReturned)),
            _ => tables.remove(0),
        };

        #[derive(Debug, FromDao)]
        struct ColumnSpec {
            schema: String,
            table_name: String,
            name: String,
            comment: String,
            type_: String,
        }

        let columns: Vec<ColumnDef> = self
            .execute_sql_with_return(
                r#"
                SELECT TABLE_SCHEMA AS `schema`,
                       TABLE_NAME AS table_name,
                       COLUMN_NAME AS name,
                       COLUMN_COMMENT AS comment,
                       CAST(COLUMN_TYPE as CHAR(255)) AS type_
                  FROM INFORMATION_SCHEMA.COLUMNS
                 WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?"#,
                &[&table_spec.schema.clone().into(), table_name],
            )?
            .iter()
            .map(|dao| FromDao::from_dao(&dao))
            .map(|spec: ColumnSpec| {
                let (sql_type, capacity) =
                    if spec.type_.starts_with("enum(") || spec.type_.starts_with("set(") {
                        let start = spec.type_.find('(');
                        let end = spec.type_.find(')');
                        if let (Some(start), Some(end)) = (start, end) {
                            let dtype = &spec.type_[0..start];
                            println!("dtype: {:?}", dtype);
                            let range = &spec.type_[start + 1..end];
                            let choices = range
                                .split(',')
                                .map(|v| v.to_owned())
                                .collect::<Vec<String>>();

                            match dtype {
                                "enum" => (SqlType::Enum(dtype.to_owned(), choices), None),
                                "set" => (SqlType::Enum(dtype.to_owned(), choices), None),
                                _ => panic!("not yet handled: {}", dtype),
                            }
                        } else {
                            panic!("not yet handled spec_type: {:?}", spec.type_)
                        }
                    } else {
                        let (dtype, capacity) = common::extract_datatype_with_capacity(&spec.type_);
                        let sql_type = match &*dtype {
                            "tinyint" | "tinyint unsigned" => SqlType::Tinyint,
                            "smallint" | "smallint unsigned" | "year" => SqlType::Smallint,
                            "mediumint" | "mediumint unsigned" => SqlType::Int,
                            "int" | "int unsigned" => SqlType::Int,
                            "bigint" | "bigin unsigned" => SqlType::Bigint,
                            "float" | "float unsigned" => SqlType::Float,
                            "double" | "double unsigned" => SqlType::Double,
                            "decimal" => SqlType::Numeric,
                            "tinyblob" => SqlType::Tinyblob,
                            "mediumblob" => SqlType::Mediumblob,
                            "blob" => SqlType::Blob,
                            "longblob" => SqlType::Longblob,
                            "binary" | "varbinary" => SqlType::Varbinary,
                            "char" => SqlType::Char,
                            "varchar" => SqlType::Varchar,
                            "tinytext" => SqlType::Tinytext,
                            "mediumtext" => SqlType::Mediumtext,
                            "text" | "longtext" => SqlType::Text,
                            "date" => SqlType::Date,
                            "datetime" | "timestamp" => SqlType::Timestamp,
                            "time" => SqlType::Time,
                            _ => panic!("not yet handled: {}", dtype),
                        };

                        (sql_type, capacity)
                    };

                ColumnDef {
                    table: TableName::from(&format!("{}.{}", spec.schema, spec.table_name)),
                    name: ColumnName::from(&spec.name),
                    comment: Some(spec.comment),
                    specification: column::ColumnSpecification {
                        capacity,
                        // TODO: implementation
                        constraints: vec![],
                        sql_type,
                    },
                    stat: None,
                }
            })
            .collect();

        Ok(Some(TableDef {
            name: TableName {
                name: table_spec.name,
                schema: Some(table_spec.schema),
                alias: None,
            },
            comment: Some(table_spec.comment),
            columns,
            is_view: table_spec.is_view == 1,
            // TODO: implementation
            table_key: vec![],
        }))
    }

    fn get_tablenames(&mut self) -> Result<Vec<TableName>, DbError> {
        #[derive(Debug, FromDao)]
        struct TableNameSimple {
            table_name: String,
        }
        let sql =
            "SELECT TABLE_NAME as table_name FROM information_schema.tables WHERE TABLE_SCHEMA = database()";

        let rows: Rows = self.execute_sql_with_return(sql, &[])?;
        println!("rows: {:#?}", rows);

        let result: Vec<TableNameSimple> = self
            .execute_sql_with_return(sql, &[])?
            .iter()
            .map(|row| TableNameSimple {
                table_name: row.get("table_name").expect("must have a table name"),
            })
            .collect();
        let tablenames = result
            .iter()
            .map(|r| TableName::from(&r.table_name))
            .collect();
        Ok(tablenames)
    }

    fn get_all_tables(&mut self) -> Result<Vec<TableDef>, DbError> {
        let tablenames = self.get_tablenames()?;
        Ok(tablenames
            .iter()
            .filter_map(|tablename| self.get_table(tablename).ok().flatten())
            .collect())
    }

    fn get_grouped_tables(&mut self) -> Result<Vec<SchemaContent>, DbError> {
        let table_names = get_table_names(&mut *self, &"BASE TABLE".to_string())?;
        let view_names = get_table_names(&mut *self, &"VIEW".to_string())?;
        let schema_content = SchemaContent {
            schema: "".to_string(),
            tablenames: table_names,
            views: view_names,
        };
        Ok(vec![schema_content])
    }

    fn get_database_name(&mut self) -> Result<Option<DatabaseName>, DbError> {
        let sql = "SELECT database() AS name";
        let mut database_names: Vec<Option<DatabaseName>> =
            self.execute_sql_with_return(sql, &[]).map(|rows| {
                rows.iter()
                    .map(|row| {
                        row.get_opt("name")
                            .expect("must not error")
                            .map(|name| DatabaseName {
                                name,
                                description: None,
                            })
                    })
                    .collect()
            })?;

        if database_names.len() > 0 {
            Ok(database_names.remove(0))
        } else {
            Ok(None)
        }
    }

    #[cfg(feature = "db-auth")]
    fn get_users(&mut self) -> Result<Vec<User>, DbError> {
        let sql = "SELECT USER as usernameFROM information_schema.user_attributes";
        let rows: Result<Rows, DbError> = self.execute_sql_with_return(sql, &[]);

        rows.map(|rows| {
            rows.iter()
                .map(|row| User {
                    //FIXME; this should be option
                    sysid: 0,
                    username: row.get("username").expect("username"),
                    //TODO: join to the user_privileges tables
                    is_superuser: false,
                    is_inherit: false,
                    can_create_db: false,
                    can_create_role: false,
                    can_login: false,
                    can_do_replication: false,
                    can_bypass_rls: false,
                    valid_until: None,
                    conn_limit: None,
                })
                .collect()
        })
    }

    #[cfg(feature = "db-auth")]
    fn get_user_detail(&mut self, _username: &str) -> Result<Vec<User>, DbError> {
        todo!()
    }

    #[cfg(feature = "db-auth")]
    fn get_roles(&mut self, _username: &str) -> Result<Vec<Role>, DbError> {
        todo!()
    }

    fn set_autoincrement_value(
        &mut self,
        _table_name: &TableName,
        _sequence_value: i64,
    ) -> Result<Option<i64>, DbError> {
        todo!()
    }

    fn get_autoincrement_last_value(
        &mut self,
        _table_name: &TableName,
    ) -> Result<Option<i64>, DbError> {
        todo!()
    }
}

fn get_table_names(db: &mut dyn Database, kind: &str) -> Result<Vec<TableName>, DbError> {
    #[derive(Debug, FromDao)]
    struct TableNameSimple {
        table_name: String,
    }
    let sql = "SELECT TABLE_NAME as table_name FROM information_schema.tables WHERE table_type= ?";
    let result: Vec<TableNameSimple> = db
        .execute_sql_with_return(sql, &[&kind.to_value()])?
        .iter()
        .map(|row| TableNameSimple {
            table_name: row.get("table_name").expect("must have a table name"),
        })
        .collect();
    let mut table_names = vec![];
    for r in result {
        let table_name = TableName::from(&r.table_name);
        table_names.push(table_name);
    }
    Ok(table_names)
}

#[derive(Debug)]
pub struct MyValue<'a>(&'a Value);

impl mysql::prelude::ToValue for MyValue<'_> {
    fn to_value(&self) -> mysql::Value {
        match self.0 {
            Value::Bool(ref v) => v.into(),
            Value::Tinyint(ref v) => v.into(),
            Value::Smallint(ref v) => v.into(),
            Value::Int(ref v) => v.into(),
            Value::Bigint(ref v) => v.into(),
            Value::Float(ref v) => v.into(),
            Value::Double(ref v) => v.into(),
            Value::Blob(ref v) => v.into(),
            Value::Char(ref v) => v.to_string().into(),
            Value::Text(ref v) => v.into(),
            Value::Uuid(ref v) => v.as_bytes().into(),
            Value::Date(ref v) => v.into(),
            Value::Timestamp(ref v) => v.naive_utc().into(),
            Value::DateTime(ref v) => v.into(),
            Value::Time(ref v) => v.into(),
            Value::Interval(ref _v) => panic!("storing interval in DB is not supported"),
            Value::Json(ref v) => v.into(),
            Value::Nil => mysql::Value::NULL,
            Value::BigDecimal(_) => unimplemented!("we need to upgrade bigdecimal crate"),
            Value::Point(_) | Value::Array(_) => unimplemented!("unsupported type"),
        }
    }
}

fn into_record(
    mut row: mysql::Row,
    column_types: &[mysql::consts::ColumnType],
) -> Result<Vec<Value>, MysqlError> {
    use mysql::{consts::ColumnType, from_value_opt as fvo};

    column_types
        .iter()
        .enumerate()
        .map(|(i, column_type)| {
            let cell: mysql::Value = row
                .take_opt(i)
                .unwrap_or_else(|| unreachable!("column length does not enough"))
                .unwrap_or_else(|_| unreachable!("could not convert as `mysql::Value`"));

            if cell == mysql::Value::NULL {
                return Ok(Value::Nil);
            }

            match column_type {
                ColumnType::MYSQL_TYPE_DECIMAL | ColumnType::MYSQL_TYPE_NEWDECIMAL => fvo(cell)
                    .and_then(|v: Vec<u8>| {
                        bigdecimal::BigDecimal::parse_bytes(&v, 10)
                            .ok_or(mysql::FromValueError(mysql::Value::Bytes(v)))
                    })
                    .map(Value::BigDecimal),
                ColumnType::MYSQL_TYPE_TINY => fvo(cell).map(Value::Tinyint),
                ColumnType::MYSQL_TYPE_SHORT | ColumnType::MYSQL_TYPE_YEAR => {
                    fvo(cell).map(Value::Smallint)
                }
                ColumnType::MYSQL_TYPE_LONG | ColumnType::MYSQL_TYPE_INT24 => {
                    fvo(cell).map(Value::Int)
                }
                ColumnType::MYSQL_TYPE_LONGLONG => fvo(cell).map(Value::Bigint),
                ColumnType::MYSQL_TYPE_FLOAT => fvo(cell).map(Value::Float),
                ColumnType::MYSQL_TYPE_DOUBLE => fvo(cell).map(Value::Double),
                ColumnType::MYSQL_TYPE_NULL => fvo(cell).map(|_: mysql::Value| Value::Nil),
                ColumnType::MYSQL_TYPE_TIMESTAMP => fvo(cell).map(|v: chrono::NaiveDateTime| {
                    Value::Timestamp(chrono::DateTime::from_utc(v, chrono::Utc))
                }),
                ColumnType::MYSQL_TYPE_DATE | ColumnType::MYSQL_TYPE_NEWDATE => {
                    fvo(cell).map(Value::Date)
                }
                ColumnType::MYSQL_TYPE_TIME => fvo(cell).map(Value::Time),
                ColumnType::MYSQL_TYPE_DATETIME => fvo(cell).map(Value::DateTime),
                ColumnType::MYSQL_TYPE_VARCHAR
                | ColumnType::MYSQL_TYPE_VAR_STRING
                | ColumnType::MYSQL_TYPE_STRING => fvo(cell).map(Value::Text),
                ColumnType::MYSQL_TYPE_JSON => fvo(cell).map(Value::Json),
                ColumnType::MYSQL_TYPE_TINY_BLOB
                | ColumnType::MYSQL_TYPE_MEDIUM_BLOB
                | ColumnType::MYSQL_TYPE_LONG_BLOB
                | ColumnType::MYSQL_TYPE_BLOB => fvo(cell).map(Value::Blob),
                ColumnType::MYSQL_TYPE_TIMESTAMP2
                | ColumnType::MYSQL_TYPE_DATETIME2
                | ColumnType::MYSQL_TYPE_TIME2 => {
                    panic!("only used in server side: {:?}", column_type)
                }
                ColumnType::MYSQL_TYPE_BIT
                | ColumnType::MYSQL_TYPE_ENUM
                | ColumnType::MYSQL_TYPE_SET
                | ColumnType::MYSQL_TYPE_GEOMETRY
                | ColumnType::MYSQL_TYPE_TYPED_ARRAY
                | ColumnType::MYSQL_TYPE_UNKNOWN => {
                    panic!("not yet handling this kind: {:?}", column_type)
                }
            }
            .map_err(MysqlError::from)
        })
        .collect()
}

#[derive(Debug, Error)]
pub enum MysqlError {
    #[error("{0}")]
    UrlError(#[from] mysql::UrlError),
    #[error("Error executing {1}: {0}")]
    SqlError(mysql::Error, String),
    #[error("{0}")]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("{0}")]
    ConvertError(#[from] mysql::FromValueError),
    #[error("Pool initialization error: {0}")]
    PoolInitializationError(#[from] r2d2::Error),
}

impl From<mysql::Error> for MysqlError {
    fn from(e: mysql::Error) -> Self {
        MysqlError::SqlError(e, "Generic Error".into())
    }
}
