use std::collections::HashMap;

static CONN_ID : std::sync::Mutex<u64> = std::sync::Mutex::new(0);

fn alloc_conn_id() -> u64 {
	let mut guard = CONN_ID.lock().unwrap();
	let curr_val = *guard;
	*guard = *guard + 1;
	curr_val
}

pub struct ConnectionData {

	pub conn_id : u64,

	pub remote_ip            : String,
	pub connected_to         : String,
	pub bytes_to_remote_ip   : usize,
	pub bytes_to_fowarded_ip : usize,

	pub active: bool,
}

impl ConnectionData {
	fn new(id : u64) -> ConnectionData {
		return ConnectionData {
			conn_id : id,

			remote_ip : "".to_string(),
			connected_to : "".to_string(),
			bytes_to_remote_ip : 0,
			bytes_to_fowarded_ip : 0,

			active : true,
		};
	}
}

pub struct Connections {
	pub connections : HashMap<u64, ConnectionData>,
}

impl Connections {

	pub fn new () -> Connections {
		return Connections { 
			connections: HashMap::new() 
		};
	}

	pub fn add(&mut self) -> &mut ConnectionData {
		let id = alloc_conn_id();

		if let Some(_) = &self.connections.get(&id){
			panic!("Already contains this id!");
		} else {}

		let cn = ConnectionData::new(id);
		self.connections.insert(id, cn);

		let tmp = self.connections.get_mut(&id).unwrap();
		tmp
	}	

	pub fn get(&mut self, id : u64) -> &mut ConnectionData {
		let tmp = self.connections.get_mut(&id).unwrap();
		tmp
	}

}