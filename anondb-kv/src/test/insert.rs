use futures::TryStreamExt;

use super::*;

#[domacro(all_read_write_impls)]
async fn insert_table<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    let existing = handle
        .insert(&table_name, key.as_slice(), val.as_slice())
        .await?;
    assert!(existing.is_none());

    let new = handle
        .get(&table_name, key.as_slice())
        .await?
        .expect("Document was not inserted");
    assert_eq!(new.value().await, val.to_vec());

    assert_eq!(handle.count(&table_name).await?, 1);
    // assert_eq!(
    //     handle
    //         .range(&table_name, ..)
    //         .await?
    //         .collect::<Vec<_>>()
    //         .len(),
    //     1
    // );

    Ok(())
}

#[domacro(all_read_write_impls)]
async fn insert_multimap_table<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    handle
        .insert_multimap(&table_name, key.as_slice(), val.as_slice())
        .await?;

    let new = handle
        .get_multimap(&table_name, key.as_slice())
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    assert_eq!(new.len(), 1);
    let item = new.first().unwrap();
    assert_eq!(item.key(), key);
    assert_eq!(item.value().await, val);

    assert_eq!(handle.count_multimap(&table_name).await?, 1);
    // assert_eq!(
    //     handle
    //         .range_multimap(&table_name, ..)
    //         .await?
    //         .collect::<Vec<_>>()
    //         .len(),
    //     1
    // );

    Ok(())
}
