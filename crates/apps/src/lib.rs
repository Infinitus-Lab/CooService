//! 应用管理：`app` 表是唯一真相，载入成内存里的 [`instance::AppInstance`]。

pub mod channel;
pub mod error;
pub mod instance;
pub mod manager;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
