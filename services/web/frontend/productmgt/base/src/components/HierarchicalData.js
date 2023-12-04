import React, {Component} from 'react';
import SortableTree, {addNodeUnderParent, getNodeAtPath, removeNode, walk, getTreeFromFlatData} from 'react-sortable-tree';
import 'react-sortable-tree/style.css';

import {BaseUrl, EMPTY_VALUES, DEFAULT_API_REQUEST_OPTIONS} from '../js/constants.js';
import {clear_array, clear_props_object, APIconsumer} from '../js/common/native.js';


export default class HierarchicalData extends Component {
    constructor(props) {
        super(props);
        // TODO, load initial tree data from backend
        this.state = {
            treeData: [
                //{title:'food',   name:'food',  id:278, expanded:false, subtitle:'eat/drink/cook', children:[]},
                //{title:'cooker', name:'cooker', id:51, expanded:false, subtitle:'cook helper', children:[]},
                //{title:'oven',   name:'fitz',  id:620 , expanded:false, subtitle:'cook helper', children:[]},
            ],
        };
        this._uncommitted_nodes = {added: [],   edited: {},  removed: {},};
        this.node_ro_fields = ['children', 'expanded', '_dirty', 'parentId'];
        this._refresh_ro_fields = ['desc_cnt', ...props.refresh_ro_fields];
        this._search_ro_fields  = ['desc_cnt', ...props.search_ro_fields];
        this._app_node_id_label = props._node_id_label ? props._node_id_label: 'id';
        var api_props = {num_retry: 4, wait_interval_ms: 385};
        this.api_caller = new APIconsumer(api_props);
        // the response callback of fetch() API call will ignore all the functions
        // imported from third-party libraries , to use these 3rd-party functions
        // in the promise / response callback, I have to bind these functions to
        // this object
        this.bound_tree_ops = {load:getTreeFromFlatData , get_node:getNodeAtPath,
            clear_array:clear_array, clear_props_object:clear_props_object }
    }
    
    setState(new_state) {
        Component.prototype.setState.call(this, new_state);
    } // end of setState
    
    on_state_change(treeData) {
        this.setState({treeData: treeData}); // implicitly refresh the tree view
    }

    getNodeAppID(node, default_value) {
        // ID applied in user application
        var out = default_value; // defaults to undefined
        if(node){
            out = node[ this._app_node_id_label ];
            if(EMPTY_VALUES.indexOf(out) >= 0) {
                out = default_value;
            }
        }
        return out;
    }

    setNodeAppID(node_a, node_b) {
        if(node_a && node_b){
            node_a[this._app_node_id_label] = node_b[this._app_node_id_label];
        }
    }

    _render_read_child_api(node) {
        let _id = this.getNodeAppID(node);
        let url = _id ? this.props.api_base_url.singular_desc.format(_id) : "";
        return url;
    }

    getNodeKey(node) {
        // it seems like good practice to always use tree index
        // as key of each node, because tree index is always there.
        // If you use different field in node object as a node key
        // (for example  node.my_id , node.uuid ... etc), you need
        // to ensure that the field (inside the node) always exists.
        return node.treeIndex;
    }

    get_node(path) {
        // path has to be an array of index numbers that represent a loaded node
        return this.bound_tree_ops.get_node({
            treeData: this.state.treeData,
            path    : path,
            getNodeKey: this.getNodeKey.bind(this)
        });
    }

    _push_to_remove_list(node_data) {
        var removed = this._uncommitted_nodes.removed;
        var _id = this.getNodeAppID(node_data);
        if(_id !== undefined && removed[_id] === undefined) {
            removed[_id] = node_data;
        }
    }

    _delete_all_nodes_local(evt) {
        // fetch all nodes that were already saved at backend
        // , gather the node data, for making DELETE API call later
        walk({
            treeData: this.state.treeData,
            getNodeKey: this.getNodeKey,
            ignoreCollapsed: false,
            callback: (node) => {
                this._push_to_remove_list(node.node);
            }
        });
        return {treeData: []};
    }

    _delete_node_local(evt) {
        // delete given single node locally
        var path = evt.nativeEvent.target.dataset.node_key_path;
        path = path.split(',').map(key => parseInt(key));
        var new_tree = removeNode({
            treeData: this.state.treeData,
            path    : path,
            getNodeKey: this.getNodeKey
        });
        if (!new_tree.node) {
            var err_msg = "the node under the path "+ path +" cannot be removed";
            throw Error(err_msg);
        }
        this._push_to_remove_list(new_tree.node);
        return {treeData: new_tree.treeData};
    }

    _add_node_local(form) {
        let path = form.state.node_key_path;
        let curr_node_key = path ? path[path.length - 1] : null;
        let NEW_NODE_DATA = form.get_data();
        NEW_NODE_DATA._dirty = true;
        NEW_NODE_DATA.children = [];
        //NEW_NODE_DATA.parentId = ;
        var new_tree = addNodeUnderParent({
            treeData: this.state.treeData,
            newNode: NEW_NODE_DATA,
            expandParent: true,
            parentKey: curr_node_key, // added as root node if undefined
            getNodeKey: this.getNodeKey
        });
        if (new_tree.treeIndex === undefined || new_tree.treeIndex === null) {
            var err_msg = "failed to add the new node ";
            throw Error(err_msg);
        }
        return {treeData: new_tree.treeData};
    }

    _edit_node_local(form) {
        let path = form.state.node_key_path;
        let found_node = this.get_node(path);
        let node_data = found_node.node;
        let EDIT_NODE = form.get_data();
        for(var field_name in EDIT_NODE) {
            if (this.node_ro_fields.indexOf(field_name) < 0) {
                node_data[field_name] = EDIT_NODE[field_name];
            }
        }
        node_data._dirty = true;
        return {treeData: this.state.treeData};
    }
    
    update_tree_status(evt) {
        var new_state = undefined;
        if (evt.nativeEvent.target.dataset.removenode === "true") {
            if (evt.nativeEvent.target.dataset.removeall === "true") {
                new_state = this._delete_all_nodes_local(evt);
            } else {
                new_state = this._delete_node_local(evt);
            }
        } else {  // check whether the caller attempts to create a new node or not
            const form = this.props.form_ref.current;
            if(form.state.addnode) { // create new node
                new_state = this._add_node_local(form);
            } else { // edit one node
                new_state = this._edit_node_local(form);
            }
        }
        this.setState(new_state);
    } // end of update_tree_status
   

    save(urlpath, callbacks) {
        var skipped = true;
        this._gather_unsaved_nodes();
        // append internal callback to the end of callback list for clearing
        //  uncommitted nodes when the API endpoint is successfully called
        var callbacks_copied = APIconsumer.copy_callbacks(callbacks, ['succeed']);
        callbacks_copied.succeed.push(this._save_callback_succeed.bind(this));
        // TODO: 2 test cases
        // #1 : make 3 seperate API calls for deleted nodes, added nodes, edited nodes
        // #2 : commit all deleted/added/edited nodes in one go 
        var url = BaseUrl.API_HOST + urlpath ;
        var req_opt = {
            post: DEFAULT_API_REQUEST_OPTIONS.POST(),
            delete: DEFAULT_API_REQUEST_OPTIONS.DELETE(),
        };
        // get valid field names from the form, for fetching node value
        const form = this.props.form_ref.current;
        var valid_field_names = form.get_field_names();
        var valid_field_names_post = [...valid_field_names];
        valid_field_names_post.push('exist_parent');
        valid_field_names_post.push('new_parent');
        req_opt.post.body   = this._serialize_nodes(this._uncommitted_nodes.added,   valid_field_names_post);
        req_opt.delete.body = this._serialize_nodes(this._uncommitted_nodes.removed, valid_field_names);
        // run test case #1
        if(req_opt.post.body) {
            this.api_caller.start({base_url:url, req_opt:req_opt.post, callbacks:callbacks_copied,
                params:{fields:this._app_node_id_label}});
            skipped = false;
        } else {
            skipped = this._call_edit_node_api(url, callbacks_copied);
        }
        if(req_opt.delete.body) {
            this.api_caller.start({base_url:url, req_opt:req_opt.delete, params:null, callbacks:callbacks_copied});
            skipped = false;
        }
        return skipped;
    }  // end of save()

    _call_edit_node_api(url, callbacks) {
        // may be forced to save all edited node after all added nodes are saved at backend
        var skipped = true;
        const form = this.props.form_ref.current;
        var valid_field_names = form.get_field_names();
        valid_field_names.push('exist_parent');
        var req_opt = {
            put:  DEFAULT_API_REQUEST_OPTIONS.PUT(),
        };
        req_opt.put.body = this._serialize_nodes(this._uncommitted_nodes.edited,  valid_field_names);
        if(req_opt.put.body) {
            this.api_caller.start({base_url:url, req_opt:req_opt.put , params:null, callbacks:callbacks});
            skipped = false;
        }
        return skipped;
    }
    
    _gather_unsaved_nodes() {
        // clear all items that were previously added
        clear_array(this._uncommitted_nodes.added);
        clear_props_object(this._uncommitted_nodes.edited);
        walk({
            treeData: this.state.treeData,
            getNodeKey: this.getNodeKey,
            ignoreCollapsed: false,
            callback: this._gather_unsaved_nodes_treewalk_callback.bind(this)
        });
    } // end of  _gather_unsaved_nodes()
    
    _gather_unsaved_nodes_treewalk_callback(node) {
        var added  = this._uncommitted_nodes.added;
        var edited = this._uncommitted_nodes.edited;
        var node_data = node.node;
        if (!node_data._dirty) {
            return;
        }
        var _id = this.getNodeAppID(node_data);
        var parent_id = this.getNodeAppID(node.parentNode);
        delete node_data.exist_parent;
        delete node_data.new_parent;
        if(parent_id) {
            node_data.exist_parent = parent_id;
        } else if (node.parentNode) {
            // due to the fact that tree walk always starts from top node to the bottom,
            // I can simply iterate added list and check whether the new parent node
            // is already added to the list. (it should be)
            var new_parent_idx = -1;
            for(var idx = 0; idx < added.length; idx++) {
                if(added[idx] === node.parentNode) {
                    new_parent_idx = idx;
                    break;
                }
            }
            if(new_parent_idx < 0) {
                throw Error('node_data.new_parent has NOT been added to the list');
            }
            node_data.new_parent = new_parent_idx;
        }
        if(_id !== undefined && edited[_id] === undefined) {
            edited[_id] = node_data;
        } else if(_id === undefined) {
            added.push(node_data);
        }
    } // end of _gather_unsaved_nodes_treewalk_callback
    
    on_move_node(kwargs) {
        // mark as dirty for existing node (note that newly added nodes
        // are already dirty)
        var curr_node = kwargs.node;
        var id = this.getNodeAppID(curr_node);
        if(id) {
            curr_node._dirty = true;
        }
    }

    _serialize_nodes(nodeset, valid_field_names) {
        return APIconsumer.serialize(nodeset, valid_field_names);
    }

    _save_callback_succeed(data, res, props) {
        var added  = this._uncommitted_nodes.added;
        var edited = this._uncommitted_nodes.edited;
        var edited_array = Object.entries(edited).map(kv => kv[1]);
        var req_args = res.req_args;
        var req_mthd = req_args.req_opt.method ;
        var clear_dirty_flag_fn = (nodedata) => { delete nodedata._dirty };
        // for added, edited nodes, clear dirty flag of uncommitted nodes in the list
        if(req_mthd === 'POST') {
            // Update new IDs of added nodes
            // Note that API endpoint must respond with ID of the newly-added nodes
            for(var idx = 0; idx < added.length; idx++) {
                this.setNodeAppID(added[idx], data[idx]);
            }
            edited_array.map((nodedata) => {
                if(nodedata.new_parent >= 0) {
                    var new_parent_node = added[nodedata.new_parent];
                    nodedata.exist_parent = this.getNodeAppID( new_parent_node ); // must not be undefined
                    delete nodedata.new_parent;
                } // resolve parent of each node for add-edit dependency
                return nodedata;
            });
            added.map(clear_dirty_flag_fn);
            clear_array(added);
            this._call_edit_node_api(req_args.base_url, req_args.callbacks);
        } else if(req_mthd === 'PUT') {
            // TODO, return number of children for parents of edited nodes
            edited_array.map(clear_dirty_flag_fn);
            clear_props_object(edited);
        } else if(req_mthd === 'DELETE') {
            var removed = this._uncommitted_nodes.removed;
            clear_props_object(removed);
        } else if(req_mthd === 'PATCH') {
            // TODO, implement 3-in-1 API endpoint (add/edit/delete in one go)
        }
    } // end of _save_callback_succeed
    
    refresh(evt) {
        var num_saved_child = evt.nativeEvent.target.dataset.num_saved_child;
        // refresh one layer of immediate children of a given node
        let api_url = evt.nativeEvent.target.dataset.api_url;
        let depth   = evt.nativeEvent.target.dataset.depth;
        let node_key_path = evt.nativeEvent.target.dataset.node_key_path;
        if (api_url.indexOf('root') < 0 && (!num_saved_child || num_saved_child <= 0)) {
            console.log('no saved child node to fetch under the given node ');
            return;
        }
        if(api_url && api_url.length > 0) {
            // get valid field names from the form, then add extra read-only fields at backend
            const form = this.props.form_ref.current;
            let valid_field_names = [...form.get_field_names(), ...this._refresh_ro_fields];
            let query_params = {depth:depth, fields: valid_field_names, ordering: '-name'}
            let req_opt      = DEFAULT_API_REQUEST_OPTIONS.GET();
            let callbacks    = {
                succeed    : [this._refresh_callback_succeed.bind(this)],
                server_busy: [this._refresh_callback_server_busy.bind(this)],
                unhandled_error_response: [this._refresh_callback_unhandled_error_response.bind(this)],
                unhandled_exception:      [this._refresh_callback_unhandled_exception.bind(this)],
            };
            let extra = {node_key_path:node_key_path};
            this.api_caller.start({base_url: BaseUrl.API_HOST + api_url, req_opt:req_opt,
                callbacks:callbacks,  params: query_params, extra:extra});
        } else {
            console.log("this node hasn't been saved to backend yet, it cannot be refreshed ");
        }
    } // end of refresh

    _refresh_callback_succeed(data, res, props) {
        if (!data || (data && data.result === undefined && data.length === 0) ||
                (data && data.result !== undefined && data.results.length === 0)) {
            console.log('succeed callback, no descendants to fetch under the given node ');
            return;
        }
        let path = res.req_args.extra.node_key_path;
        let found_node_data = null;
        if(path !== undefined) {
            path =   path.split(',').map(key => parseInt(key));
            found_node_data = this.get_node(path).node;
        }
        let new_tree = [];
        data = data.map((item) => {
            item.title = item.name;
            item.expanded = true; 
            item.children = [];
            item.parentId = found_node_data ? this.getNodeAppID(found_node_data) : null;
            return item;
        })
        if (found_node_data) {
            new_tree = this.state.treeData;
            this.bound_tree_ops.clear_array(found_node_data.children);
            found_node_data.children.push(...data)
            // TODO: clean up duplicate nodes which were already added in edit list,
            // (existing nodes shouldn't be in create list, which doesn't make sense)
        } else { // remove all modified nodes
            new_tree = data;
            this.bound_tree_ops.clear_array(this._uncommitted_nodes.added);
            this.bound_tree_ops.clear_props_object(this._uncommitted_nodes.edited);
            this.bound_tree_ops.clear_props_object(this._uncommitted_nodes.removed);
        }
        //var dbg_array = Object.entries(data[0]).map(kv => kv[1]);
        //console.log("HierarchicalData._refresh_callback_succeed invoked: "+ dbg_array );
        //console.log("path : "+ path +", found_node_data : "+ found_node_data);
        //console.log("this.state.treeData : "+ this.state.treeData);
        this.setState({treeData: new_tree});
    } // end of _refresh_callback_succeed

    _refresh_callback_server_busy(data, res, props) {
        console.log("HierarchicalData._refresh_callback_server_busy invoked");
    } // end of _refresh_callback_server_busy
    
    _refresh_callback_unhandled_error_response(data, res, props) {
        // let new_tree = this.bound_tree_ops.load({
        //     flatData: [
        //         {title:'tesla',  name:'nutLLa',  id:180, parentId:null, subtitle:'eat/drink/cook',},
        //         {title:'toxic',  name:'workenv', id:29 , parentId:null, subtitle:'cook helper', },
        //     ],
        //     getKey: this.getNodeAppID.bind(this) , // id or index ?
        //     // getParentKey: (node) => node.parentId, // default is `parentId`
        //     rootKey: null, // must not be `undefined` , but it can be null
        // });
        console.log("HierarchicalData._refresh_callback_unhandled_error_response invoked : ");
    } // end of _refresh_callback_unhandled_error_response
    
    _refresh_callback_unhandled_exception(data, res, props) {
        console.log("HierarchicalData._refresh_callback_unhandled_exception invoked");
    } // end of _refresh_callback_unhandled_exception

    search(keyword, api_url) {
        const form = this.props.form_ref.current;
        let valid_field_names = [...form.get_field_names(), ...this._search_ro_fields];
        let query_params = {fields: valid_field_names, ordering: '-item_cnt',
                search: keyword, parent_only:'yes'}
        let req_opt    = DEFAULT_API_REQUEST_OPTIONS.GET();
        let callbacks  = {
            succeed    : [this._search_callback_succeed.bind(this)],
            server_busy: [this._refresh_callback_server_busy.bind(this)],
            unhandled_error_response: [this._refresh_callback_unhandled_error_response.bind(this)],
            unhandled_exception:      [this._refresh_callback_unhandled_exception.bind(this)],
        };
        let extra = {};
        this.api_caller.start({base_url: BaseUrl.API_HOST + api_url, req_opt:req_opt,
            callbacks:callbacks,  params: query_params, extra:extra});
    } // end of search

    _search_callback_succeed(data, res, props){
        if (!data || data.length === 0) {
            console.log('succeed callback, no descendants to fetch with the search keyword ...');
            return;
        }
        data = data.map((item) => {
            item.title = item.name;
            item.expanded = true; 
            item.children = [];
            if (item.ancestors.length > 0) {
                item.parentId = item.ancestors[0].ancestor.id;
            } else {
                item.parentId = null;
            }
            return item;
        })
        let new_tree = this.bound_tree_ops.load({
            flatData: data,
            getKey: this.getNodeAppID.bind(this) , // id or index ?
            getParentKey: (node) => node.parentId, // default is `parentId`
            rootKey: null, // must not be `undefined` , but it can be null
        });
        this.setState({treeData: new_tree});
        this.bound_tree_ops.clear_array(this._uncommitted_nodes.added);
        this.bound_tree_ops.clear_props_object(this._uncommitted_nodes.edited);
        this.bound_tree_ops.clear_props_object(this._uncommitted_nodes.removed);
    }

    render() {
        // * SortableTree is a function components, it does NOT have instance of React component
        //   so you cannot pass React.createRef() to SortableTree directly
        // * There is unknown bug between react.js (> 17.0.0) and react-sortable-tree,
        //   the workaround found by github community member is to temporarily set the property
        //   `isVirtualized` to false in order to disable some of less important features , perhaps
        //   this bug will be fixed in future version
        return <SortableTree isVirtualized={ false }
                treeData={this.state.treeData}
                onChange={this.on_state_change.bind(this)}
                onMoveNode={this.on_move_node.bind(this)}
                generateNodeProps={
                    ({node, path, treeidx}) => ({
                        // path is a list of tree indexes which represent ancestors of a node
                        title: node.title,
                        subtitle: "#child, saved: "+ (node.desc_cnt? node.desc_cnt: 'NaN') +
                            " , present: "+ node.children.length,
                        buttons: [
                            <button className='btn btn-primary'  data-toggle="modal"
                                data-target={"#"+ this.props.edit_form_id}
                                data-addnode="true" data-addchild="true"
                                data-node_key_path={ path.join(',') }
                                onClick={ this.props.fp_render_form }
                            >
                                add child
                            </button> ,
                            <button className='btn btn-primary'  data-toggle="modal"
                                data-target={"#"+ this.props.edit_form_id}
                                data-node_key_path={ path.join(',') }
                                data-addnode="false"  onClick={ this.props.fp_render_form }
                            >
                                edit
                            </button> ,
                            <button className='btn btn-primary'   data-node_key_path={ path.join(',') }
                                data-removenode="true"  onClick={ this.update_tree_status.bind(this) }
                            >
                                remove
                            </button> ,
                            <button className='btn btn-primary' data-node_key_path={ path.join(',') }
                                 data-api_url={ this._render_read_child_api(node) }
                                 data-num_saved_child={node.desc_cnt} data-depth="1"
                                 onClick={this.refresh.bind(this)}
                            >
                                refresh
                            </button> ,
                        ], // end of button list for each node
                    })
                } // end of generateNodeProps
            /> ;
    } // end of render function
} // end of class


