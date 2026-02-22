//! Basic integration test: use rtmalloc as the global allocator and exercise
//! standard Rust collections.

use rtmalloc::RtMalloc;

#[global_allocator]
static GLOBAL: RtMalloc = RtMalloc;

#[test]
fn test_box() {
    let b = Box::new(42u64);
    assert_eq!(*b, 42);
    drop(b);
}

#[test]
fn test_vec() {
    let mut v = Vec::new();
    for i in 0..1000 {
        v.push(i);
    }
    assert_eq!(v.len(), 1000);
    assert_eq!(v[500], 500);
    v.clear();
}

#[test]
fn test_string() {
    let mut s = String::new();
    for _ in 0..100 {
        s.push_str("hello world ");
    }
    assert!(s.len() > 100);
}

#[test]
fn test_hashmap() {
    use std::collections::HashMap;
    let mut map = HashMap::new();
    for i in 0..500 {
        map.insert(i, format!("value_{}", i));
    }
    assert_eq!(map.len(), 500);
    assert_eq!(map[&42], "value_42");
}

#[test]
fn test_vec_of_strings() {
    let v: Vec<String> = (0..200).map(|i| format!("item_{}", i)).collect();
    assert_eq!(v.len(), 200);
    assert_eq!(v[100], "item_100");
}

#[test]
fn test_nested_collections() {
    let mut v: Vec<Vec<u32>> = Vec::new();
    for i in 0..50 {
        let inner: Vec<u32> = (0..i).collect();
        v.push(inner);
    }
    assert_eq!(v[49].len(), 49);
}

#[test]
fn test_large_allocation() {
    // Allocate > 256 KiB (goes through large allocation path)
    let v: Vec<u8> = vec![0xAB; 512 * 1024];
    assert_eq!(v.len(), 512 * 1024);
    assert!(v.iter().all(|&b| b == 0xAB));
}

#[test]
fn test_various_sizes() {
    // Exercise different size classes
    let _a: Box<[u8; 1]> = Box::new([0; 1]);
    let _b: Box<[u8; 8]> = Box::new([0; 8]);
    let _c: Box<[u8; 16]> = Box::new([0; 16]);
    let _d: Box<[u8; 64]> = Box::new([0; 64]);
    let _e: Box<[u8; 256]> = Box::new([0; 256]);
    let _f: Box<[u8; 1024]> = Box::new([0; 1024]);
    let _g: Box<[u8; 4096]> = Box::new([0; 4096]);
    let _h: Box<[u8; 8192]> = Box::new([0; 8192]);
    let _i: Box<[u8; 65536]> = Box::new([0; 65536]);
}

#[test]
fn test_alloc_free_cycle() {
    for _ in 0..100 {
        let v: Vec<u64> = (0..100).collect();
        assert_eq!(v.len(), 100);
        drop(v);
    }
}
