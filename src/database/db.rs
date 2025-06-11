use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use diesel_async::AsyncPgConnection;
        use diesel_async::pooled_connection::AsyncDieselConnectionManager;
        use diesel_async::pooled_connection::deadpool::Pool;

        pub type DbPool = Pool<AsyncPgConnection>;

        pub fn establish_connection(database_url: &str) -> Result<DbPool, Box<dyn std::error::Error>> {
            let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
            let pool = Pool::builder(config)
                .max_size(8)  // Set max pool size
                .build()?;
            Ok(pool)
        }
}}
