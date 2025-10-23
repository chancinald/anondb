use super::*;

#[domacro(all_read_write_impls)]
fn clear_nonexistent<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);

    handle.clear(&table_name)?;

    Ok(())
}

#[domacro(all_read_write_impls)]
fn clear_nonexistent_multimap<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);

    handle.clear_multimap(&table_name)?;

    Ok(())
}

#[domacro(all_read_write_impls)]
fn clear<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    handle.insert(&table_name, key.as_slice(), val.as_slice())?;

    assert_eq!(handle.count(&table_name)?, 1);

    handle.clear(&table_name)?;

    assert_eq!(handle.count(&table_name)?, 0);

    Ok(())
}

#[domacro(all_read_write_impls)]
fn clear_multimap<T: ReadOperations + WriteOperations>(handle: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    let val = rand::random::<[u8; 32]>();

    handle.insert_multimap(&table_name, key.as_slice(), val.as_slice())?;

    assert_eq!(handle.count_multimap(&table_name)?, 1);

    handle.clear_multimap(&table_name)?;

    assert_eq!(handle.count_multimap(&table_name)?, 0);

    Ok(())
}
