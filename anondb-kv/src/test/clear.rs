use super::*;

#[domacro(all_read_write_impls)]
async fn clear_nonexistent<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    handle.clear(&table_name).await?;
    Ok(())
}

#[domacro(all_read_write_impls)]
async fn clear_nonexistent_multimap<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    handle.clear_multimap(&table_name).await?;
    Ok(())
}

#[domacro(all_read_write_impls)]
async fn clear<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    handle
        .insert(&table_name, key.as_slice(), val.as_slice())
        .await?;

    assert_eq!(handle.count(&table_name).await?, 1);

    handle.clear(&table_name).await?;

    assert_eq!(handle.count(&table_name).await?, 0);

    Ok(())
}

#[domacro(all_read_write_impls)]
async fn clear_multimap<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    handle
        .insert_multimap(&table_name, key.as_slice(), val.as_slice())
        .await?;

    assert_eq!(handle.count_multimap(&table_name).await?, 1);

    handle.clear_multimap(&table_name).await?;

    assert_eq!(handle.count_multimap(&table_name).await?, 0);

    Ok(())
}
