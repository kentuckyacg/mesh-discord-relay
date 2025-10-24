use sqlx::{query_as, Pool, Sqlite};

pub struct NodeInfo {
    pub idx: i64,
    pub name: String,
    pub nodeid: i64,
}

pub struct Count {
    count: i64,
}

pub async fn init(db_url: String) -> Result<Pool<Sqlite>, String> {
    let db_pool = match Pool::connect(db_url.as_str()).await {
        Ok(pool) => pool,
        Err(e) => {
            return Err(format!("Failed to connect to database: {}", e));
        }
    };

    let res = query_as!(
        NodeInfo,
        "create table if not exists node_info (
            idx INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            name text not null,
            nodeid INTEGER not null
        )",
    ).execute(&db_pool).await;

    if let Err(e) = res {
        return Err(format!("Failed to create database table: {}", e));
    }

    Ok(db_pool)
}

pub async fn get_node_name(db_pool: &Pool<Sqlite>, node_id: u32) -> Result<String, String> {
    let res = match query_as!(
        NodeInfo,
        "select * from node_info where nodeid = $1",
        node_id
    ).fetch_one(db_pool).await {
        Ok(n) => n,
        Err(e) => return Err(format!("Failed to fetch node_info: {}", e)),
    };

    return Ok(res.name);
}

pub async fn add_node_name(db_pool: &Pool<Sqlite>, name: String, node_id: u32) -> Result<(), String> {
    let count = match query_as!(
        Count,
        "select COUNT(*) as count from node_info where nodeid = $1",
        node_id
    ).fetch_one(db_pool).await {
        Ok(c) => c,
        Err(e) => return Err(format!("Failed to fetch node_info: {}", e)),
    }.count;

    if count > 0 {
        return Ok(());
    }

    let res = query_as!(
        NodeInfo,
        "insert into node_info (name, nodeid) values ($1, $2)",
        name, node_id
    ).execute(db_pool).await;

    if let Err(e) = res {
        return Err(format!("Failed to insert node_info: {}", e));
    };

    Ok(())
}