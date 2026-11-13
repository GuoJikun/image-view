//! 后台任务代数（generation）。
//!
//! 快速切换图片/目录时，只有最新一代请求可以更新界面；后台线程完成旧任务后
//! 由调用方用 [`Generation::is_current`] 丢弃结果。这里不包含线程实现。

/// 单调递增的请求代号。
#[derive(Debug, Default)]
pub struct Generation(u64);

impl Generation {
    /// 推进到下一代并返回新代号。
    pub fn advance(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }

    /// 当前代号。
    pub fn current(&self) -> u64 {
        self.0
    }

    /// 完成的结果是否仍属于当前一代。
    pub fn is_current(&self, generation: u64) -> bool {
        generation == self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_advances_and_rejects_stale_results() {
        let mut generation = Generation::default();
        let first = generation.advance();
        let second = generation.advance();

        assert!(!generation.is_current(first));
        assert!(generation.is_current(second));
        assert_eq!(generation.current(), second);
    }
}
