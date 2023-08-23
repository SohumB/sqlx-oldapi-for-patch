use sqlx::any::AnyRow;
use sqlx::{Any, Connection, Decode, Executor, Row, Type};
use sqlx_test::new;

async fn get_val<T>(expr: &str) -> anyhow::Result<T>
where
    for<'r> T: Decode<'r, Any> + Type<Any> + std::marker::Unpin + std::marker::Send + 'static,
{
    let mut conn = new::<Any>().await?;
    let val = sqlx::query(&format!("select {}", expr))
        .try_map(|row: AnyRow| row.try_get::<T, _>(0))
        .fetch_one(&mut conn)
        .await?;
    conn.close().await?;
    Ok(val)
}

#[sqlx_macros::test]
async fn it_has_all_the_types() -> anyhow::Result<()> {
    assert_eq!(6, get_val::<i32>("5 + 1").await?);
    assert_eq!(1234567890123, get_val::<i64>("1234567890123").await?);
    assert_eq!(
        "hello world".to_owned(),
        get_val::<String>("'hello world'").await?
    );
    assert_eq!(None, get_val::<Option<i32>>("NULL").await?);
    assert_eq!(1e-40, get_val::<f64>("1e-40").await?);
    assert_eq!(12.5f64, get_val::<f64>("CAST(12.5 AS DECIMAL(9,2))").await?);
    assert_eq!(
        125125.125f64,
        get_val::<f64>("CAST(125125.125 AS DECIMAL(10,3))").await?
    );
    assert_eq!(
        -1234567890.125,
        get_val::<f64>("CAST(-1234567890.125 AS DECIMAL(15,3))").await?
    );
    Ok(())
}

#[cfg(feature = "chrono")]
#[sqlx_macros::test]
async fn it_has_chrono() -> anyhow::Result<()> {
    use sqlx::types::chrono::NaiveDate;
    assert_eq!(
        NaiveDate::from_ymd_opt(2020, 1, 2).unwrap(),
        get_val::<NaiveDate>("CAST('20200102' AS DATE)").await?
    );
    Ok(())
}

#[cfg(feature = "bigdecimal")]
#[sqlx_macros::test]
async fn it_has_bigdecimal() -> anyhow::Result<()> {
    use sqlx::types::BigDecimal;
    use std::str::FromStr;
    assert_eq!(
        BigDecimal::from_str("1234567.25")?,
        get_val::<BigDecimal>("CAST('1234567.25' AS DECIMAL(9,2))").await?
    );
    Ok(())
}

#[cfg(feature = "decimal")]
#[sqlx_macros::test]
async fn it_has_decimal() -> anyhow::Result<()> {
    use sqlx::types::Decimal;
    use std::str::FromStr;
    assert_eq!(
        Decimal::from_str("1234567.25")?,
        get_val::<Decimal>("CAST('1234567.25' AS DECIMAL(9,2))").await?
    );
    Ok(())
}

#[cfg(feature = "json")]
#[sqlx_macros::test]
async fn it_has_json() -> anyhow::Result<()> {
    use serde_json::json;
    assert_eq!(
        json!({"foo": "bar"}),
        get_val::<serde_json::Value>(
            // SQLite and Mssql do not have a native JSON type, strings are parsed as JSON
            if cfg!(any(feature = "sqlite", feature = "mssql")) {
                "'{\"foo\": \"bar\"}'"
            } else {
                "CAST('{\"foo\": \"bar\"}' AS JSON)"
            }
        )
        .await?
    );
    Ok(())
}

#[sqlx_macros::test]
async fn it_pings() -> anyhow::Result<()> {
    let mut conn = new::<Any>().await?;

    conn.ping().await?;

    Ok(())
}

#[sqlx_macros::test]
async fn it_executes_with_pool() -> anyhow::Result<()> {
    let pool = sqlx_test::pool::<Any>().await?;

    let rows = pool.fetch_all("SELECT 1; SElECT 2").await?;

    assert_eq!(rows.len(), 2);

    Ok(())
}

#[sqlx_macros::test]
async fn it_does_not_stop_stream_after_decoding_error() -> anyhow::Result<()> {
    use futures::stream::StreamExt;
    // see https://github.com/launchbadge/sqlx/issues/1884
    let pool = sqlx_test::pool::<Any>().await?;

    #[derive(Debug, PartialEq)]
    struct MyType;
    impl<'a> sqlx::FromRow<'a, AnyRow> for MyType {
        fn from_row(row: &'a AnyRow) -> sqlx::Result<Self> {
            let n = row.try_get::<i32, _>(0)?;
            if n == 1 {
                Err(sqlx::Error::RowNotFound)
            } else {
                Ok(MyType)
            }
        }
    }

    let rows = sqlx::query_as("SELECT 0 UNION ALL SELECT 1 UNION ALL SELECT 2")
        .fetch(&pool)
        .map(|r| r.ok())
        .collect::<Vec<_>>()
        .await;

    assert_eq!(rows, vec![Some(MyType), None, Some(MyType)]);
    Ok(())
}

#[sqlx_macros::test]
async fn it_gets_by_name() -> anyhow::Result<()> {
    let mut conn = new::<Any>().await?;

    let row = conn.fetch_one("SELECT 1 as _1").await?;
    let val: i32 = row.get("_1");

    assert_eq!(val, 1);

    Ok(())
}

#[sqlx_macros::test]
async fn it_can_fail_and_recover() -> anyhow::Result<()> {
    let mut conn = new::<Any>().await?;

    for i in 0..10 {
        // make a query that will fail
        let res = conn
            .execute("INSERT INTO not_found (column) VALUES (10)")
            .await;

        assert!(res.is_err());

        // now try and use the connection
        let val: i32 = conn
            .fetch_one(&*format!("SELECT {}", i))
            .await?
            .get_unchecked(0);

        assert_eq!(val, i);
    }

    Ok(())
}

#[sqlx_macros::test]
async fn it_can_fail_and_recover_with_pool() -> anyhow::Result<()> {
    let pool = sqlx_test::pool::<Any>().await?;

    for i in 0..10 {
        // make a query that will fail
        let res = pool
            .execute("INSERT INTO not_found (column) VALUES (10)")
            .await;

        assert!(res.is_err());

        // now try and use the connection
        let val: i32 = pool
            .fetch_one(&*format!("SELECT {}", i))
            .await?
            .get_unchecked(0);

        assert_eq!(val, i);
    }

    Ok(())
}
