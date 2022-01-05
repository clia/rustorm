use rustorm::{
    dao,
    ColumnName,
    Dao,
    DbError,
    FromDao,
    Pool,
    TableName,
    ToColumnNames,
    ToTableName,
};

#[derive(Debug, FromDao, ToColumnNames, ToTableName)]
#[allow(dead_code)]
struct Actor {
    actor_id: i32,
    first_name: String,
}

fn main() {
    let db_url = "postgres://postgres:p0stgr3s@localhost/sakila";
    let mut pool = Pool::new();
    let mut em = pool.em(db_url).unwrap();
    let sql = "SELECT * FROM actor LIMIT 10";
    let actors: Result<Vec<Actor>, DbError> = em.execute_sql_with_return(sql, &[]);
    println!("Actor: {:#?}", actors);
    let actors = actors.unwrap();
    assert_eq!(actors.len(), 10);
    for actor in actors {
        println!("actor: {:?}", actor);
    }
}
