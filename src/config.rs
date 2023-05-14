
#[derive(Clone)]
pub struct ForwardInfo {
	pub listen_to : String,
	pub forward_to : String, 
	pub whitelist : Vec<String>
}

pub struct Config {
	pub forward_info : Vec<ForwardInfo>
}

impl Config {

	pub fn from_file(file: &str) -> Option<Config> {
		let data = std::fs::read_to_string(file).ok()?;
		let jso : serde_json::Value = serde_json::from_str(&data).ok()?;
		let jso = jso.as_array()?;

		let mut ret_val = Config {
			forward_info : Vec::new()
		};

		for config_elem in jso {
			let config_elem = config_elem.as_object()?;

			let listen_to = &config_elem.get("listen_to")?.as_str()?;
			let forward_to = &config_elem.get("forward_to")?.as_str()?;
			let wl = config_elem.get("whitelist")?.as_array()?;
			let wl = wl.iter().map(|val| val.as_str()).collect::<Option<Vec<&str>>>()?;
			let wl = wl.iter().map(|val| val.to_string()).collect::<Vec<String>>();

			let fw_info = ForwardInfo {
				listen_to : listen_to.to_string(),
				forward_to : forward_to.to_string(),
				whitelist : wl
			};

			ret_val.forward_info.push(fw_info);
		}

		Some(ret_val)
	}

	pub fn _is_whitelisted(&self, _remote : &str) -> bool {
		true
	}
}