import unittest

from ecommerce_common.util.graph import is_graph_cyclic, path_exists


class CycleDetectionTestCase(unittest.TestCase):
    def setUp(self):
        pass

    def tearDown(self):
        pass

    def test_acylic_graph(self):
        graph = {
            "15": {"outbound": {"16", "18"}, "inbound": {"86", "19"}},
            "86": {"outbound": {"15"}, "inbound": []},
            "2": {"outbound": set(), "inbound": ["14"]},
            "14": {"outbound": {"91", "2"}, "inbound": []},
            "41": {"outbound": set(), "inbound": ["16"]},
            "16": {"outbound": {"41"}, "inbound": ["15"]},
            "91": {"outbound": set(), "inbound": ["14"]},
            "18": {"outbound": set(), "inbound": ["15"]},
            "19": {"outbound": {"15"}, "inbound": set()},
        }
        has_cycle, loop_node_list = is_graph_cyclic(graph=graph)
        self.assertFalse(has_cycle)
        self.assertEqual(loop_node_list, None)
        has_cycle, loop_node_list = is_graph_cyclic(graph=graph, is_directed=True)
        self.assertFalse(has_cycle)
        self.assertEqual(loop_node_list, None)

    def test_tree(self):
        graph = {
            2: {"outbound": {3, 4}, "inbound": set()},
            3: {"outbound": {5, 6, 9}, "inbound": {2}},
            4: {"outbound": {7, 8, 10, 11}, "inbound": {2}},
            5: {"outbound": set(), "inbound": {3}},
            6: {"outbound": set(), "inbound": {3}},
            7: {"outbound": set(), "inbound": {4}},
            8: {"outbound": set(), "inbound": {4}},
            9: {"outbound": set(), "inbound": {3}},
            10: {"outbound": set(), "inbound": {4}},
            11: {"outbound": set(), "inbound": {4}},
        }
        has_cycle, loop_node_list = is_graph_cyclic(graph=graph)
        self.assertFalse(has_cycle)
        self.assertEqual(loop_node_list, None)
        has_cycle, loop_node_list = is_graph_cyclic(graph=graph, is_directed=True)
        self.assertFalse(has_cycle)
        self.assertEqual(loop_node_list, None)

    def test_cylic_graph(self):
        graph = {
            2: {"outbound": {2}, "inbound": {2}},
        }
        has_cycle, loop_node_list = is_graph_cyclic(graph=graph, is_directed=True)
        self.assertTrue(has_cycle)
        self.assertIn(
            2,
            loop_node_list,
        )
        graph = {
            2: {"outbound": {3}, "inbound": {3}},
            3: {"outbound": {2}, "inbound": {2}},
        }
        has_cycle, loop_node_list = is_graph_cyclic(graph=graph, is_directed=True)
        self.assertTrue(has_cycle)
        self.assertIn(
            2,
            loop_node_list,
        )
        self.assertIn(
            3,
            loop_node_list,
        )
        graph = {
            2: {"outbound": {3}, "inbound": {4}},
            3: {"outbound": {4}, "inbound": {2}},
            4: {"outbound": {2}, "inbound": {3}},
        }
        has_cycle, loop_node_list = is_graph_cyclic(graph=graph, is_directed=True)
        self.assertTrue(has_cycle)
        self.assertIn(
            2,
            loop_node_list,
        )
        self.assertIn(
            3,
            loop_node_list,
        )
        self.assertIn(
            4,
            loop_node_list,
        )


class PathExistTestCase(unittest.TestCase):
    def setUp(self):
        pass

    def tearDown(self):
        pass

    def test_path_exists(self):
        graph = {
            2: {"outbound": {3, 4}, "inbound": set()},
            3: {"outbound": {5, 6, 9}, "inbound": {2}},
            4: {"outbound": {7, 8, 10, 11}, "inbound": {2}},
            5: {"outbound": set(), "inbound": {3}},
            6: {"outbound": set(), "inbound": {3}},
            7: {"outbound": set(), "inbound": {4}},
            8: {"outbound": set(), "inbound": {4}},
            9: {"outbound": set(), "inbound": {3}},
            10: {"outbound": set(), "inbound": {4}},
            11: {"outbound": set(), "inbound": {4}},
        }
        result = path_exists(graph=graph, node_src=2, node_dst=9)
        self.assertTrue(result)
        result = path_exists(graph=graph, node_src=9, node_dst=2)
        self.assertFalse(result)
