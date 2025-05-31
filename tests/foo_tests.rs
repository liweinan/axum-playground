#[tokio::test]
async fn test_move() {
    let x = String::from("hello");
    let closure = move || drop(x);
    closure();
    // closure(); // 错误：x 已消耗
}