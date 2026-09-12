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
