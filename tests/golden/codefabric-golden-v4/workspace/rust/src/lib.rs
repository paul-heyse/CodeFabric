pub fn double(value: i32) -> i32 { // @anchor rust.double
    value * 2
}

pub fn pipeline(value: i32) -> i32 { // @anchor rust.pipeline
    double(value) + 1 // @anchor rust.call.double
}

pub fn choose(flag: bool, left: i32, right: i32) -> i32 { // @anchor rust.choose
    if flag { left } else { right } // @anchor rust.branch
}
