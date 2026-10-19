use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct PluginKey(pub(crate) usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ExecutionPathId(pub(crate) u64);
pub(crate) struct ReadyQueue<T> {
	callers: RoundRobin<PluginKey, RoundRobin<ExecutionPathId, VecDeque<T>>>,
}

impl<T> Default for ReadyQueue<T> {
	fn default() -> Self {
		Self {
			callers: RoundRobin::default(),
		}
	}
}

impl<T> ReadyQueue<T> {
	pub(crate) fn enqueue(&mut self, caller: PluginKey, path: ExecutionPathId, call: T) {
		self.callers = std::mem::take(&mut self.callers).update(caller, |paths| {
			paths.update(path, |mut calls| {
				calls.push_back(call);
				calls
			})
		});
	}

	pub(crate) fn dequeue(&mut self) -> Option<T> {
		let (caller, mut paths) = self.callers.pop()?;
		let (path, mut calls) = paths.pop()?;
		let call = calls.pop_front();
		let paths = if calls.is_empty() {
			paths
		} else {
			paths.insert(path, calls)
		};
		self.callers = if paths.is_empty() {
			std::mem::take(&mut self.callers)
		} else {
			std::mem::take(&mut self.callers).insert(caller, paths)
		};
		call
	}
}

struct RoundRobin<K, V> {
	order: VecDeque<K>,
	values: HashMap<K, V>,
}

impl<K, V> Default for RoundRobin<K, V> {
	fn default() -> Self {
		Self {
			order: VecDeque::new(),
			values: HashMap::new(),
		}
	}
}

impl<K: Clone + Eq + Hash, V> RoundRobin<K, V> {
	fn insert(mut self, key: K, value: V) -> Self {
		if !self.values.contains_key(&key) {
			self.order.push_back(key.clone());
		}
		self.values.insert(key, value);
		self
	}

	fn update(mut self, key: K, update: impl FnOnce(V) -> V) -> Self
	where
		V: Default,
	{
		let value = if let Some(value) = self.values.remove(&key) {
			value
		} else {
			self.order.push_back(key.clone());
			V::default()
		};
		self.values.insert(key, update(value));
		self
	}

	fn pop(&mut self) -> Option<(K, V)> {
		let key = self.order.pop_front()?;
		self.values.remove(&key).map(|value| (key, value))
	}

	fn is_empty(&self) -> bool {
		self.order.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::{ExecutionPathId, PluginKey, ReadyQueue};
	#[test]
	fn rotates_callers_then_paths_preserving_fifo() {
		let mut queue = ReadyQueue::default();
		queue.enqueue(PluginKey(1), ExecutionPathId(1), "a1-first");
		queue.enqueue(PluginKey(1), ExecutionPathId(1), "a1-second");
		queue.enqueue(PluginKey(1), ExecutionPathId(2), "a2");
		queue.enqueue(PluginKey(2), ExecutionPathId(3), "b");
		let output: Vec<_> = std::iter::from_fn(|| queue.dequeue()).collect();
		assert_eq!(output, ["a1-first", "b", "a2", "a1-second"]);
		queue.enqueue(PluginKey(1), ExecutionPathId(1), "again");
		assert_eq!(queue.dequeue(), Some("again"));
		assert_eq!(queue.dequeue(), None);
	}
	#[test]
	fn busy_paths_do_not_starve_other_callers_or_paths() {
		let mut queue = ReadyQueue::default();
		for n in 0..100 {
			queue.enqueue(PluginKey(1), ExecutionPathId(1), n);
		}
		queue.enqueue(PluginKey(2), ExecutionPathId(1), 200);
		queue.enqueue(PluginKey(1), ExecutionPathId(2), 300);
		assert_eq!(queue.dequeue(), Some(0));
		assert_eq!(queue.dequeue(), Some(200));
		assert_eq!(queue.dequeue(), Some(300));
		assert_eq!(queue.dequeue(), Some(1));
	}
}
