import pytest
from jwst_py import Workspace


def test_workspace():
    ws = Workspace("test")
    assert ws.id == "test"
    assert ws.block_count() == 0

    ws.create("affine", "test")
    ws.create("affine", "test1")

    assert ws.block_count() == 1
    assert ws.exists("affine") == True

    ws.create("jwst", "test")
    ws.create("jwst", "test1")
    assert ws.block_count() == 2
    assert ws.exists("jwst") == True
