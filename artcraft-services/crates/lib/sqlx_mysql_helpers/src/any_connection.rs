use sqlx::{MySql, MySqlPool};
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;

// TODO: I don't think I finished this work.

/// Pass a handle to MySql without caring about the type.
pub enum MySqlHandle {
  ConnectionPool(MySqlPool),
  Connection(PoolConnection<MySql>),
}

/// Pass a handle to MySql without caring about the type.
/// NB: The connection is typically owned, whereas the connection pool is not.
pub enum MySqlHandleMixedRef<'a> {
  ConnectionPool(&'a MySqlPool),
  Connection(PoolConnection<MySql>),
}

/// Pass a handle to MySql without caring about the type.
pub enum MySqlHandleRef<'a> {
  ConnectionPool(&'a MySqlPool),
  Connection(&'a mut PoolConnection<MySql>),
}

//impl MySqlHandle {
//  pub fn as_ref(&mut self) -> MySqlHandleRef {
//    match self {
//      Self::ConnectionPool(inner) => MySqlHandleRef::ConnectionPool(inner),
//      Self::Connection(inner) => MySqlHandleRef::Connection(inner),
//    }
//  }
//}

impl <'a> MySqlHandleMixedRef<'a> {
  /// Consume the handle and return a connection.
  pub async fn to_discrete_connection(self) -> AnyhowResult<PoolConnection<MySql>> {
    match self {
      Self::Connection(connection) => Ok(connection),
      Self::ConnectionPool(pool) => Ok(pool.acquire().await?),
    }
  }
}

//impl <'a> MySqlHandleRef<'a> {
//  /// Consume the handle and return a connection.
//  pub async fn to_discrete_connection(self) -> AnyhowResult<()> {
//    let _r = match self {
//      Self::Connection(connection) => Ok(connection),
//      Self::ConnectionPool(pool) => {
//        let connection = pool.acquire().await?;
//        Ok(connection)
//      },
//    };
//
//    Ok(())
//  }
//}
//
///// Exists to hold ownership (if necessary)
//pub struct MySqlConnectionWrapper<'a> {
//  maybe_owned: Option<PoolConnection<MySql>>,
//  reference: &'a mut PoolConnection<MySql>,
//}



// TODO:
//impl <'a> MySqlConnectionRef<'a> {
//  pub async fn get_pool_connection(&'a mut self) -> AnyhowResult<&'a PoolConnection<MySql>> {
//    match self {
//      MySqlConnectionRef::ConnectionPool(pool) => {
//        let connection = pool.acquire().await?;
//        Ok(connection)
//      }
//      MySqlConnectionRef::Connection(inner) => Ok(inner),
//    }
//
//  }
//}
