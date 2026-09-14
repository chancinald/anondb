use futures::TryStreamExt;

use super::*;

#[domacro(all_read_impls)]
async fn count_non_existent_table<T: ReadOperations>(read: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    assert_eq!(read.count(&table_name).await?, 0);
    Ok(())
}

#[domacro(all_read_impls)]
async fn get_non_existent_table<T: ReadOperations>(read: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    assert!(read.get(&table_name, key.as_slice()).await?.is_none());
    Ok(())
}

#[domacro(all_range_read_impls)]
async fn range_non_existent_table<T: RangeReadOperations>(read: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    assert!(
        read.range(&table_name, ..)
            .await?
            .try_collect::<Vec<_>>()
            .await?
            .is_empty()
    );
    Ok(())
}

#[domacro(all_read_impls)]
async fn count_non_existent_multimap_table<T: ReadOperations>(read: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    assert_eq!(read.count_multimap(&table_name).await?, 0);
    Ok(())
}

#[domacro(all_range_read_impls)]
async fn get_non_existent_multimap_table<T: RangeReadOperations>(read: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    let key = rand::random::<[u8; 32]>();
    assert!(
        read.get_multimap(&table_name, key.as_slice())
            .await?
            .try_collect::<Vec<_>>()
            .await?
            .is_empty()
    );
    Ok(())
}

#[domacro(all_range_read_impls)]
async fn range_non_existent_multimap_table<T: RangeReadOperations>(read: &T) -> Result<()> {
    let table_name = rand_utf8(10);
    assert!(
        read.range_multimap(&table_name, ..)
            .await?
            .try_collect::<Vec<_>>()
            .await?
            .is_empty()
    );
    Ok(())
}
