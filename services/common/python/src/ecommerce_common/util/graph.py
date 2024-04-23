def depth_first_search(
    graph: dict, node, visited: dict, parent: int, is_directed, options
):
    """DFS algorithm for undirected graph"""
    # DFS visits each node N if it hasn't been visited,
    # then recursively further visit all other node N_x connected with node N
    # until all nodes are visited.
    visited[node] = True
    # print( "".join(["[visit edge] : (", str(parent), ",", str(node), ")"]))
    options["node_visited"].append(node)

    def _visit_fn(adj):
        if not visited[adj]:
            depth_first_search(graph, adj, visited, node, is_directed, options)
        elif adj != parent:
            options["has_cycle"] = True

    tuple(map(_visit_fn, graph[node]["outbound"]))
    if not is_directed:
        tuple(map(_visit_fn, graph[node]["inbound"]))


def is_graph_cyclic(graph: dict, is_directed=False):
    result = False
    loop_start_node = None
    visited = {k: False for k in graph.keys()}
    subgraph = {"has_cycle": False, "node_visited": []}
    # In case we get a graph including multiple disconnected subgraphs,
    # loop through all subgraphs & check any unvisited node
    while True:
        node_id = list(visited.keys())[0]
        # treated as undirected graph, because I need to visit all nodes of the subgraphs
        depth_first_search(
            graph=graph,
            node=node_id,
            visited=visited,
            parent=-1,
            is_directed=False,
            options=subgraph,
        )
        if subgraph["has_cycle"]:
            result = True
            loop_start_node = subgraph["node_visited"]
            break
        if False in visited.values():
            # move the nodes which haven't been visited yet to beginning of the list, run DFS again in next iteration.
            visited = {
                k: v for k, v in sorted(visited.items(), key=lambda item: item[1])
            }
            subgraph["node_visited"].clear()
        else:
            break
    if result is True and is_directed is True:
        node_start = loop_start_node[0]
        result = path_exists(
            graph=graph, node_src=node_start, node_dst=node_start, is_directed=True
        )
        if result is False:
            loop_start_node = None
    return result, loop_start_node


# end of is_graph_cyclic()


def path_exists(graph: dict, node_src, node_dst, is_directed: bool = True):
    assert (
        node_src is not None and node_dst is not None
    ), "both of node_src and node_dst have to be non-null value"
    visited = {k: False for k in graph.keys()}
    subgraph = {"has_cycle": False, "node_visited": []}
    node_id = node_src
    depth_first_search(
        graph=graph,
        node=node_id,
        visited=visited,
        parent=-1,
        is_directed=is_directed,
        options=subgraph,
    )
    # path exists while both of the nodes can be visited in any subgraph of the given input graph
    result = visited[node_src] and visited[node_dst]
    return result


#### while bool(visited):
#### visited = {key:status for key, status in visited.items() if not status}
