use super::{
    block_cache_sync_all, get_block_cache, BlockDevice, DirEntry, DiskInode, DiskInodeType,
    EasyFileSystem, DIRENT_SZ,
};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};
use log::debug;
/// Virtual filesystem layer over easy-fs
pub struct Inode {
    block_id: usize,
    block_offset: usize,
    fs: Arc<Mutex<EasyFileSystem>>,
    block_device: Arc<dyn BlockDevice>,
}

impl Inode {
    /// Create a vfs inode
    pub fn new(
        block_id: u32,
        block_offset: usize,
        fs: Arc<Mutex<EasyFileSystem>>,
        block_device: Arc<dyn BlockDevice>,
    ) -> Self {
        Self {
            block_id: block_id as usize,
            block_offset,
            fs,
            block_device,
        }
    }
    /// 获取特定id
    pub fn inode_id(&self) -> u64 {
        ((self.block_id as u64) << 32) | (self.block_offset as u64)
    }
    /// Call a function over a disk inode to read it
    fn read_disk_inode<V>(&self, f: impl FnOnce(&DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .read(self.block_offset, f)
    }
    /// Call a function over a disk inode to modify it
    fn modify_disk_inode<V>(&self, f: impl FnOnce(&mut DiskInode) -> V) -> V {
        get_block_cache(self.block_id, Arc::clone(&self.block_device))
            .lock()
            .modify(self.block_offset, f)
    }
    /// Find inode under a disk inode by name
    fn find_inode_id(&self, name: &str, disk_inode: &DiskInode) -> Option<u32> {
        // assert it is a directory
        assert!(disk_inode.is_dir());
        let file_count = (disk_inode.size as usize) / DIRENT_SZ;
        let mut dirent = DirEntry::empty();
        for i in 0..file_count {
            assert_eq!(
                disk_inode.read_at(DIRENT_SZ * i, dirent.as_bytes_mut(), &self.block_device,),
                DIRENT_SZ,
            );
            if dirent.name() == name {
                return Some(dirent.inode_id() as u32);
            }
        }
        None
    }
    /// Find inode under current inode by name
    pub fn find(&self, name: &str) -> Option<Arc<Inode>> {
        let fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            self.find_inode_id(name, disk_inode).map(|inode_id| {
                let (block_id, block_offset) = fs.get_disk_inode_pos(inode_id);
                Arc::new(Self::new(
                    block_id,
                    block_offset,
                    self.fs.clone(),
                    self.block_device.clone(),
                ))
            })
        })
    }
    /// Increase the size of a disk inode
    fn increase_size(
        &self,
        new_size: u32,
        disk_inode: &mut DiskInode,
        fs: &mut MutexGuard<EasyFileSystem>,
    ) {
        if new_size < disk_inode.size {
            return;
        }
        let blocks_needed = disk_inode.blocks_num_needed(new_size);
        let mut v: Vec<u32> = Vec::new();
        for _ in 0..blocks_needed {
            v.push(fs.alloc_data());
        }
        disk_inode.increase_size(new_size, v, &self.block_device);
    }
    /// Create inode under current inode by name
    pub fn create(&self, name: &str) -> Option<Arc<Inode>> {
        let mut fs = self.fs.lock();
        let op = |root_inode: &DiskInode| {
            // assert it is a directory
            assert!(root_inode.is_dir());
            // has the file been created?
            self.find_inode_id(name, root_inode)
        };
        if self.read_disk_inode(op).is_some() {
            return None;
        }
        // create a new file
        // alloc a inode with an indirect block
        let new_inode_id = fs.alloc_inode();
        // initialize inode
        let (new_inode_block_id, new_inode_block_offset) = fs.get_disk_inode_pos(new_inode_id);
        get_block_cache(new_inode_block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(new_inode_block_offset, |new_inode: &mut DiskInode| {
                new_inode.initialize(DiskInodeType::File);
            });
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(name, new_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });

        let (block_id, block_offset) = fs.get_disk_inode_pos(new_inode_id);
        block_cache_sync_all();
        // return inode
        Some(Arc::new(Self::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        )))
        // release efs lock automatically by compiler
    }
    /// List inodes under current inode
    pub fn ls(&self) -> Vec<String> {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| {
            let file_count = (disk_inode.size as usize) / DIRENT_SZ;
            let mut v: Vec<String> = Vec::new();
            for i in 0..file_count {
                let mut dirent = DirEntry::empty();
                assert_eq!(
                    disk_inode.read_at(i * DIRENT_SZ, dirent.as_bytes_mut(), &self.block_device,),
                    DIRENT_SZ,
                );
                v.push(String::from(dirent.name()));
            }
            v
        })
    }
    /// Read data from current inode
    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> usize {
        let _fs = self.fs.lock();
        self.read_disk_inode(|disk_inode| disk_inode.read_at(offset, buf, &self.block_device))
    }
    /// Write data to current inode
    pub fn write_at(&self, offset: usize, buf: &[u8]) -> usize {
        let mut fs = self.fs.lock();
        let size = self.modify_disk_inode(|disk_inode| {
            self.increase_size((offset + buf.len()) as u32, disk_inode, &mut fs);
            disk_inode.write_at(offset, buf, &self.block_device)
        });
        block_cache_sync_all();
        size
    }
    /// Clear the data in current inode
    pub fn clear(&self) {
        let mut fs = self.fs.lock();
        self.modify_disk_inode(|disk_inode| {
            let size = disk_inode.size;
            let data_blocks_dealloc = disk_inode.clear_size(&self.block_device);
            assert!(data_blocks_dealloc.len() == DiskInode::total_blocks(size) as usize);
            for data_block in data_blocks_dealloc.into_iter() {
                fs.dealloc_data(data_block);
            }
        });
        block_cache_sync_all();
    }

    /// 为节点创建硬链接
    pub fn create_link(&self, old_name: &str, new_name: &str) -> Result<(), i32> {
        let mut fs = self.fs.lock();
        debug!("entry creare_line");
        let inode_id = self.read_disk_inode(|disk_inode| {
            self.find_inode_id(old_name, disk_inode)
        });
        debug!("get inode_id");
        if inode_id.is_none() {
            return Err(-1); // 原文件不存在
        }
        let target_inode_id = inode_id.unwrap();
        //debug!("get target_inode_id");
        // 是否存在同名文件
        /*if self.find(new_name).is_some() {
            return Err(-1);
        }*/

        // 添加目录项
        debug!("start creare directory");
        self.modify_disk_inode(|root_inode| {
            // append file in the dirent
            let file_count = (root_inode.size as usize) / DIRENT_SZ;
            let new_size = (file_count + 1) * DIRENT_SZ;
            // increase size
            self.increase_size(new_size as u32, root_inode, &mut fs);
            // write dirent
            let dirent = DirEntry::new(new_name, target_inode_id);
            root_inode.write_at(
                file_count * DIRENT_SZ,
                dirent.as_bytes(),
                &self.block_device,
            );
        });
        debug!("after modify_disk_inode");

        let (block_id, block_offset) = fs.get_disk_inode_pos(target_inode_id);
        let target_inode = Inode::new(
            block_id,
            block_offset,
            self.fs.clone(),
            self.block_device.clone(),
        );
        debug!("after modify_disk_inode, nlink: {}",  target_inode.get_nlink());
       
        target_inode.increase_nlink();  // 调用包装的方法
        debug!("after increase, nlink: {}",  target_inode.get_nlink());

        block_cache_sync_all();
        Ok(())
    }

  /// 删除一个链接
  pub fn delete_link(&self, name: &str) -> Result<(), i32> {
    let fs = self.fs.lock();
    
    let target_inode_id = self.read_disk_inode(|disk_inode| {
        self.find_inode_id(name, disk_inode)
    });
    debug!("success target_inode_id");
    if target_inode_id.is_none() {
        return Err(-1);
    }
    let target_inode_id = target_inode_id.unwrap();

    /*let (block_id, block_offset) = fs.get_disk_inode_pos(target_inode_id);
    let target_inode = Inode::new(
        block_id,
        block_offset,
        self.fs.clone(),
        self.block_device.clone(),
    );*/
    debug!("success get_disk_inode_pos");
    let mut removed = false;
    self.modify_disk_inode(|dir_inode| {
        let file_cnt = (dir_inode.size as usize) / DIRENT_SZ;
        let mut found_index = None;
        let mut dirent = DirEntry::empty();
        
        for i in 0..file_cnt {
            dir_inode.read_at(
                i * DIRENT_SZ,
                dirent.as_bytes_mut(),
                &self.block_device,
            );

            if dirent.name() == name { 
                found_index = Some(i);
                break;
            }
        }
        debug!("success for");
        if let Some(index) = found_index {
            // 用最后一个目录项覆盖当前项
            if index < file_cnt - 1 {
                let mut last_dirent = DirEntry::empty();
                dir_inode.read_at(
                    (file_cnt - 1) * DIRENT_SZ,
                    last_dirent.as_bytes_mut(),
                    &self.block_device,
                );
                dir_inode.write_at(
                    index * DIRENT_SZ,
                    last_dirent.as_bytes(),
                    &self.block_device,
                );
            }
            debug!("success found_index");
            // 减少目录大小
            dir_inode.size = ((file_cnt - 1) * DIRENT_SZ) as u32;
            removed = true;
        }
    });
    debug!("success modify_disk_inode");

    if !removed {
        return Err(-1);
    }

    let (block_id, block_offset) = fs.get_disk_inode_pos(target_inode_id);
    drop(fs);
    let target_inode = Inode::new(
        block_id,
        block_offset,
        self.fs.clone(),
        self.block_device.clone(),
    );

    target_inode.decrease_nlink();
        debug!("success get_disk_inode_pos, numlink: {}", target_inode.get_nlink());

    if target_inode.get_nlink() == 0 {
        debug!("success get_nlink");
        target_inode.clear();
        
    }
    debug!("success clear");

    block_cache_sync_all();
    Ok(())
}

    /// 获取链接计数
    pub fn get_nlink(&self) -> u32 {
        self.read_disk_inode(|disk_inode| disk_inode.get_numlink())
    }

    /// 增加链接计数
    pub fn increase_nlink(&self) {
        self.modify_disk_inode(|disk_inode| {
            disk_inode.increase_link();
        });
    }
    
    /// 减少链接计数  
    pub fn decrease_nlink(&self) {
        self.modify_disk_inode(|disk_inode| {
            disk_inode.decrease_link();
        });
    }
}