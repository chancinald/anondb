use futures::TryStreamExt;

use super::*;

#[domacro(all_read_write_impls)]
async fn remove_non_existent<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();

    let existing = handle.remove(&table_name, key.as_slice()).await?;
    assert!(existing.is_none());

    Ok(())
}

#[domacro(all_read_write_impls)]
async fn remove_non_existent_multimap<T: ReadOperations + WriteOperations>(
    handle: &T,
) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    let existing = handle
        .remove_multimap_value(&table_name, key.as_slice(), val.as_slice())
        .await?;
    assert_eq!(existing, false);

    Ok(())
}

#[domacro(all_read_write_impls)]
async fn remove<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    let existing = handle
        .insert(&table_name, key.as_slice(), val.as_slice())
        .await?;
    assert!(existing.is_none());

    let removed = handle.remove(&table_name, key.as_slice()).await?;
    assert_eq!(removed.expect("no value removed"), val);

    assert!(handle.get(&table_name, key.as_slice()).await?.is_none());

    Ok(())
}

#[domacro(all_read_write_impls)]
async fn remove_multimap<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    handle
        .insert_multimap(&table_name, key.as_slice(), val.as_slice())
        .await?;

    assert_eq!(
        handle
            .get_multimap(&table_name, key.as_slice())
            .await?
            .try_collect::<Vec<_>>()
            .await?
            .len(),
        1
    );

    let removed = handle
        .remove_multimap_value(&table_name, key.as_slice(), val.as_slice())
        .await?;
    assert!(removed);

    assert!(
        handle
            .get_multimap(&table_name, key.as_slice())
            .await?
            .try_collect::<Vec<_>>()
            .await?
            .is_empty()
    );

    Ok(())
}

#[domacro(all_read_write_impls)]
async fn remove_all_multimap<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val0 = rand::random::<[u8; 32]>();
    let val1 = rand::random::<[u8; 32]>();

    handle
        .insert_multimap(&table_name, key.as_slice(), val0.as_slice())
        .await?;
    handle
        .insert_multimap(&table_name, key.as_slice(), val1.as_slice())
        .await?;

    assert_eq!(
        handle
            .get_multimap(&table_name, key.as_slice())
            .await?
            .try_collect::<Vec<_>>()
            .await?
            .len(),
        2
    );

    handle.remove_multimap(&table_name, key.as_slice()).await?;

    assert!(
        handle
            .get_multimap(&table_name, key.as_slice())
            .await?
            .try_collect::<Vec<_>>()
            .await?
            .is_empty()
    );

    Ok(())
}
