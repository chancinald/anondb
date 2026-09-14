use super::*;

#[domacro(all_kv_impls)]
async fn should_not_list_removed_table<T: KV>(handle: &T) -> Result<()> {
    assert!(handle.list_tables().await?.is_empty());
    handle.insert("test0", &[0], &[0]).await?;
    assert_eq!(handle.list_tables().await?.len(), 1);
    handle.insert("test1", &[0], &[0]).await?;
    assert_eq!(handle.list_tables().await?.len(), 2);
    handle.remove("test0", &[0]).await?;
    assert_eq!(handle.list_tables().await?.len(), 2);
    handle.clear("test0").await?;
    assert_eq!(handle.list_tables().await?.len(), 1);
    Ok(())
}

#[domacro(all_kv_impls)]
async fn should_not_list_removed_multimap_table<T: KV>(handle: &T) -> Result<()> {
    assert!(handle.list_multimap_tables().await?.is_empty());
    handle.insert_multimap("test0", &[0], &[0]).await?;
    assert_eq!(handle.list_multimap_tables().await?.len(), 1);
    handle.insert_multimap("test1", &[0], &[0]).await?;
    assert_eq!(handle.list_multimap_tables().await?.len(), 2);
    handle.remove_multimap("test0", &[0]).await?;
    assert_eq!(handle.list_multimap_tables().await?.len(), 2);
    handle.clear_multimap("test0").await?;
    assert_eq!(handle.list_multimap_tables().await?.len(), 1);
    Ok(())
}
