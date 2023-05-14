### Introduction
**PortForwarder** is a small utility that allows you to ”forward” ports. Can be useful if you have a machine behind multiple NAT layers (for example a virtual machine hosted on your computer) and you need to access it from outside your organization.  

### Usage example
You need to create a configuration file to describe what ports to open and where to connect. 
Example config file:
`[{
		"listen_to": "0.0.0.0:22",
		"forward_to": "1.2.3.4:22",
		"whitelist": ["3.3.3.3"]
	},
	{
		"listen_to": "0.0.0.0:80",
		"forward_to": "1.2.3.4:80",
		"whitelist": []
	}]`
	
There are a few commands to view statistics about existing or previous connections. You can view a list of supported commands using **help** command from the tool's terminal. 
