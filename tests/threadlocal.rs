use nolock::thread_data::{ThreadDataStorage, storage::{List, Trie}};
use std::rc::Rc;

#[test]
fn ub_issue_2_1_list() {
    let rc = Rc::new(());
    let data = ThreadDataStorage::<List<Rc<()>>, _>::new();
    data.get_or(|| rc.clone());
    let t = std::thread::spawn(move || drop(data));
    drop(rc);
    t.join().unwrap();
}

#[test]
fn ub_issue_2_1_trie() {
    let rc = Rc::new(());
    let data = ThreadDataStorage::<Trie<Rc<()>>, _>::new();
    data.get_or(|| rc.clone());
    let t = std::thread::spawn(move || drop(data));
    drop(rc);
    t.join().unwrap();
}

#[test]
fn ub_issue_2_2() {
    let data = nolock::thread_data::ThreadData::<u8>::new();
    data.get();
}
