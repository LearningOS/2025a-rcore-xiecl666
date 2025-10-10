
///
pub struct DeadlockDetector{
	///每种资源的数目
	pub available: [isize;5],
    /// 各线程的资源分配情况 (tid, allocation)
    pub allocations: [(usize, [usize;5]);17],
	// ///
	//pub need:[(usize, [usize;5]);17]
}
impl Clone for DeadlockDetector {
    fn clone(&self) -> Self {
        Self {
            available: self.available.clone(),
            allocations: self.allocations.clone(),
			//need:self.need.clone()
        }
    }
}
impl  DeadlockDetector{
	///
	pub fn  new()->Self{
		Self{
			available:[0; 5],
			allocations:{
				let mut arr = [(0, [0; 5]); 17];
				for (i, elem) in arr.iter_mut().enumerate() {
					elem.0 = i ; // 第一个元素设为1-17
				}
				arr
			},
			// need:{
			// 	let mut arr = [(0, [0; 5]); 17];
			// 	for (i, elem) in arr.iter_mut().enumerate() {
			// 		elem.0 = i ; // 第一个元素设为1-17
			// 	}
			// 	arr
			// }
		}
	}
}