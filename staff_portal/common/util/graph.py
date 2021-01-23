


def depth_first_search(graph: dict, node: int, visited: dict, parent: int, is_directed, options):
    """ DFS algorithm for undirected graph """
    # DFS visits each node N if it hasn't been visited,
    # then recursively further visit all other node N_x connected with node N
    # until all nodes are visited.
    visited[node] = True
    #print( "".join(["[visit edge] : (", str(parent), ",", str(node), ")"]))
    options["node_visited"].append(node)
    for adj in graph[node]['outbound']:
        if not visited[adj]:
            depth_first_search(graph, adj, visited, node, is_directed, options)
        elif adj != parent:
            options["has_cycle"] = True
    if not is_directed:
        for adj in graph[node]['inbound']:
            if not visited[adj]:
                depth_first_search(graph, adj, visited, node, is_directed, options)
            elif adj != parent:
                options["has_cycle"] = True


def is_graph_cyclic(graph: dict, is_directed: bool):
    result = False
    loop_start_node = None
    graph_keys = graph.keys()

    def _init_sort_condition(graph, is_directed: bool):
        if is_directed:
            return lambda item: len(graph[item]['outbound'])
        else:
            return lambda item: len(graph[item]['outbound']) + len(graph[item]['inbound'])

    visited  = {k: False for k in sorted(graph_keys, key=_init_sort_condition(graph, is_directed), reverse=True)}
    subgraph = {"has_cycle": False, "node_visited":[]}
    # In case we get a graph including multiple disconnected subgraphs,
    # loop through all subgraphs & check any unvisited node
    while True:
        #print("".join(["[visited] : ", str(visited)]))
        node_id = list(visited.keys())[0]
        depth_first_search(graph, node_id, visited, -1, is_directed, subgraph)
        #print( "".join(["[subgraph] : ", str(subgraph)]))
        if subgraph["has_cycle"]:
            result = True
            loop_start_node = [v for v in subgraph["node_visited"]]
            break
        if False in visited.values():
            # move the nodes which haven't been visited yet to beginning of the list, run DFS again in next iteration.
            visited = {k:v for k, v in sorted(visited.items(), key=lambda item: item[1])}
            subgraph["node_visited"].clear()
        else:
            break
    return result, loop_start_node


#### while bool(visited):
    #### visited = {key:status for key, status in visited.items() if not status}
