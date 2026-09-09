pub fn increment(value: i32) -> i32 {
    value + 1
}

pub fn pipeline(value: i32) -> i32 {
    increment(value) * 2
}

pub fn invoke(callback: fn(i32) -> i32, value: i32) -> i32 {
    callback(value)
}
