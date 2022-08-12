import pytest
from pyln.testing.fixtures import *
import os

plugin_path = os.path.join(os.path.dirname(__file__), '../target/debug/rust-bcli')

def test_plugin(node_factory, bitcoind):
    node = node_factory.get_node(options={"disable-plugin": "bcli", "plugin": plugin_path})
    info = node.rpc.getinfo()
    print(info["id"])
    
