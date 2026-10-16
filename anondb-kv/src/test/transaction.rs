use futures::TryStreamExt;

use super::*;

#[domacro(all_kv_impls)]
async fn write_not_visible_until_commit<T: KV>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    let write = handle.write_tx().await?;

    write
        .insert(&table_name, key.as_slice(), val.as_slice())
        .await?;

    assert!(handle.get(&table_name, key.as_slice()).await?.is_none());
    assert_eq!(handle.count(&table_name).await?, 0);

    write.commit().await?;

    assert_eq!(
        handle
            .get(&table_name, key.as_slice())
            .await?
            .unwrap()
            .value()
            .await,
        val
    );
    assert_eq!(handle.count(&table_name).await?, 1);

    Ok(())
}

#[domacro(all_kv_impls)]
async fn write_visible_in_transaction<T: KV>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    let write = handle.write_tx().await?;

    write
        .insert(&table_name, key.as_slice(), val.as_slice())
        .await?;

    assert_eq!(
        write
            .get(&table_name, key.as_slice())
            .await?
            .expect("value should exist")
            .value()
            .await,
        val
    );
    assert_eq!(write.count(&table_name).await?, 1);

    write.commit().await?;

    assert_eq!(
        handle
            .get(&table_name, key.as_slice())
            .await?
            .unwrap()
            .value()
            .await,
        val
    );
    assert_eq!(handle.count(&table_name).await?, 1);

    Ok(())
}

#[domacro(all_kv_impls)]
async fn write_transaction_rolls_back_without_commit<T: KV>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    {
        let write = handle.write_tx().await?;

        write
            .insert(&table_name, key.as_slice(), val.as_slice())
            .await?;

        assert_eq!(
            write
                .get(&table_name, key.as_slice())
                .await?
                .expect("value should exist")
                .value()
                .await,
            val
        );
        assert_eq!(write.count(&table_name).await?, 1);
    }

    assert!(handle.get(&table_name, key.as_slice()).await?.is_none());
    assert_eq!(handle.count(&table_name).await?, 0);

    Ok(())
}

#[domacro(all_kv_impls)]
async fn read_tx_consistent_over_write<T: KV>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    handle
        .insert(&table_name, key.as_slice(), val.as_slice())
        .await?;

    let read = handle.read_tx().await?;

    assert_eq!(read.count(&table_name).await?, 1, "first read is incorrect");

    handle.remove(&table_name, key.as_slice()).await?;
    assert_eq!(handle.count(&table_name).await?, 0);

    assert_eq!(
        read.count(&table_name).await?,
        1,
        "second read is incorrect"
    );

    Ok(())
}

#[domacro(all_kv_impls)]
async fn write_not_visible_until_commit_multimap<T: KV>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    let write = handle.write_tx().await?;

    write
        .insert_multimap(&table_name, key.as_slice(), val.as_slice())
        .await?;

    assert!(
        handle
            .get_multimap(&table_name, key.as_slice())
            .await?
            .try_collect::<Vec<_>>()
            .await?
            .is_empty()
    );
    assert_eq!(handle.count_multimap(&table_name).await?, 0);

    write.commit().await?;

    assert_eq!(
        handle
            .get_multimap(&table_name, key.as_slice())
            .await?
            .try_collect::<Vec<_>>()
            .await?
            .first()
            .unwrap()
            .value()
            .await,
        val
    );
    assert_eq!(handle.count_multimap(&table_name).await?, 1);

    Ok(())
}

#[domacro(all_kv_impls)]
async fn read_tx_consistent_over_write_multimap<T: KV>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    handle
        .insert_multimap(&table_name, key.as_slice(), val.as_slice())
        .await?;

    let read = handle.read_tx().await?;

    assert_eq!(
        read.count_multimap(&table_name).await?,
        1,
        "first read is incorrect"
    );

    handle.remove_multimap(&table_name, key.as_slice()).await?;
    assert_eq!(handle.count_multimap(&table_name).await?, 0);

    assert_eq!(
        read.count_multimap(&table_name).await?,
        1,
        "second read is incorrect"
    );

    Ok(())
}

/// A second write transaction must wait without blocking the executor thread. On a single
/// threaded runtime a blocking wait would stop the first transaction from ever finishing.
fn concurrent_write_tx_on_single_thread<T: KV>(new_kv: impl FnOnce() -> T + Send + 'static) {
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let kv = new_kv();
            let first = kv.write_tx().await.unwrap();
            let second = tokio::spawn({
                let kv = kv.clone();
                async move {
                    let second = kv.write_tx().await.unwrap();
                    second.insert("table", b"second", b"2").await.unwrap();
                    second.commit().await.unwrap();
                }
            });
            // let the second writer start waiting
            tokio::task::yield_now().await;
            first.insert("table", b"first", b"1").await.unwrap();
            first.commit().await.unwrap();
            second.await.unwrap();
            assert_eq!(kv.count("table").await.unwrap(), 2);
        });
        done_tx.send(()).ok();
    });
    assert!(
        done_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .is_ok(),
        "write transactions deadlocked"
    );
}

#[test]
fn concurrent_write_tx_memory() {
    concurrent_write_tx_on_single_thread(MemoryKV::default);
}

#[cfg(feature = "redb")]
#[test]
fn concurrent_write_tx_redb() {
    concurrent_write_tx_on_single_thread(|| {
        let path = std::env::temp_dir().join(format!("anondb-test-{}.redb", rand::random::<u64>()));
        futures::executor::block_on(RedbKV::new(Some(Box::new(path)))).unwrap()
    });
}

/// A committed write must be visible to new reads even while the next writer is halfway
/// through starting its transaction (holding the tables but not yet the read snapshot).
#[tokio::test(flavor = "multi_thread")]
async fn memory_kv_commit_visible_while_next_writer_starts() -> Result<()> {
    let kv = MemoryKV::default();
    let tx = kv.write_tx().await?;
    tx.insert("table", b"key", b"value").await?;
    tx.commit().await?;

    // holding the multimap tables stops the next writer after it locks the main tables
    let multi_guard = kv.tables_multi.clone().read_owned().await;
    let writer = tokio::spawn({
        let kv = kv.clone();
        async move { kv.write_tx().await.map(|_| ()) }
    });
    while kv.tables.try_read().is_ok() {
        tokio::task::yield_now().await;
    }
    let value = kv.read_tx().await?.get("table", b"key").await?;
    drop(multi_guard);
    writer.await??;
    assert_eq!(value.map(|item| item.value), Some(b"value".to_vec()));
    Ok(())
}
