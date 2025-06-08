use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use diesel::prelude::*;
        use diesel_async::{AsyncPgConnection, RunQueryDsl};
        use diesel_async::pooled_connection::AsyncDieselConnectionManager;
        use diesel_async::pooled_connection::deadpool::Pool;
        use crate::models::conversations::{NewMessage, Thread, Message};
        use crate::schema::{threads, messages};

        pub type DbPool = Pool<AsyncPgConnection>;

        pub fn establish_connection(database_url: &str) -> Result<DbPool, Box<dyn std::error::Error>> {
            let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
            let pool = Pool::builder(config)
                .max_size(8)  // Set max pool size
                .build()?;
            Ok(pool)
        }

        pub async fn create_thread(pool: &DbPool, new_thread: &Thread) -> Result<usize, diesel::result::Error> {
            let mut conn = pool.get().await.map_err(|e| {
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UnableToSendCommand,
                    Box::new(format!("Pool error: {e}"))
                )
            })?;

            diesel::insert_into(threads::table)
                .values(new_thread)
                .execute(&mut conn)
                .await
        }

        pub async fn add_message(pool: &DbPool, new_message: &NewMessage) -> Result<usize, diesel::result::Error> {
            let mut conn = pool.get().await.map_err(|e| {
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UnableToSendCommand,
                    Box::new(format!("Pool error: {e}"))
                )
            })?;

            diesel::insert_into(messages::table)
                .values(new_message)
                .execute(&mut conn)
                .await
        }

        pub async fn get_messages_by_thread(pool: &DbPool, thread_id: &str) -> Result<Vec<Message>, diesel::result::Error> {
            let mut conn = pool.get().await.map_err(|e| {
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UnableToSendCommand,
                    Box::new(format!("Pool error: {e}"))
                )
            })?;

            messages::table
                .filter(messages::thread_id.eq(thread_id))
                .load::<Message>(&mut conn)
                .await
        }

//        // Optional: Function to run multiple operations in a transaction
//        pub async fn create_thread_with_message(
//            pool: &DbPool,
//            new_thread: &Thread,
//            new_message: &NewMessage
//        ) -> Result<(), diesel::result::Error> {
//            let mut conn = pool.get().await.map_err(|e| {
//                diesel::result::Error::DatabaseError(
//                    diesel::result::DatabaseErrorKind::UnableToSendCommand,
//                    Box::new(format!("Pool error: {}", e))
//                )
//            })?;
//
//            conn.transaction::<_, diesel::result::Error, _>(|conn| async move {
//                diesel::insert_into(threads::table)
//                    .values(new_thread)
//                    .execute(conn)
//                    .await?;
//
//                diesel::insert_into(messages::table)
//                    .values(new_message)
//                    .execute(conn)
//                    .await?;
//
//                Ok(())
//            }.scope_boxed()).await
//        }
    }
}
