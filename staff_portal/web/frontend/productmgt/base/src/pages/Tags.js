import React from 'react';
import {Link} from 'react-router-dom';
import HierarchicalData from '../components/HierarchicalData.js';
import TagContentEditForm from '../components/TagContentEditForm.js';

let api_base_url = {
    plural: '/product/tags',
    singular: '/product/tag/{0}',
    singular_desc: '/product/tag/{0}/descendants',
    singular_asc:  '/product/tag/{0}/ancestors',
};
let refs = {hier_data: React.createRef(), tag_edit_form: React.createRef(),};

function  _render_editform_modal(evt) {
    let invoker = evt.nativeEvent.target;
    let form_ref  = this.current;
    let new_state = {
        addnode  : (invoker.dataset.addnode  === 'true' ? true: false),
        addchild : (invoker.dataset.addchild === 'true' ? true: false),
        node_key_path : invoker.dataset.node_key_path,
        callback_submit:  _update_tree_status.bind(refs.hier_data) ,
    };
    if(new_state.node_key_path) {
        var _path = new_state.node_key_path.split(',');
        new_state.node_key_path = _path.map(key => parseInt(key));
    }
    if(new_state.addnode === false) {
        var tree_ref  = refs.hier_data.current;
        var node      = tree_ref.get_node( new_state.node_key_path );
        form_ref.set_data( node.node );
    } else if (new_state.addnode === true) {
        form_ref.set_data({}); // clean up all fields in the form
    }
    form_ref.setState(new_state); // implicitly re-render the modal form
}

function _update_tree_status(evt) {
    let tree_ref = this.current;
    tree_ref.update_tree_status(evt);
}

function _refresh_root_nodes(evt) {
    let tree_ref = this.current;
    tree_ref.refresh(evt);
}

function _save_tree_status(evt) {
    let tree_ref = this.current;
    // * collect list of added nodes and edited nodes
    // * serialize them, make API call (to backend server) and wait for response
    //   (meanwhile lock the tree view to avoid user access)
    // * prompt the response (from the backend) and unlock the tree view
    var callbacks = {
        succeed: [_save_tree_callback_succeed],
        server_busy: [_save_tree_callback_server_busy],
        unhandled_error_response: [_save_tree_callback_unhandled_error_response],
        unhandled_exception: [_save_tree_callback_unhandled_exception],
    }
    tree_ref.save(api_base_url.plural, callbacks);
}

function _save_tree_callback_succeed(data, res, props) {
    console.log('_save_tree_callback_succeed get called');
}

function _save_tree_callback_server_busy(data, res, props) {
    console.log('_save_tree_callback_server_busy get called');
}

function _save_tree_callback_unhandled_error_response(data, res, props) {
    console.log('_save_tree_callback_unhandled_error_response get called');
}

function _save_tree_callback_unhandled_exception(data, res, props) {
    var req_mthd = res.req_args.req_opt.method;
    console.log('_save_tree_callback_unhandled_exception get called, method:'+ req_mthd);
}


const Tags = (props) => {
    const form_id = '_tag_edit_form';
    let hier_data = <HierarchicalData  ref={refs.hier_data}  edit_form_id={form_id}
                        fp_render_form={ _render_editform_modal.bind(refs.tag_edit_form) }
                        form_ref={ refs.tag_edit_form }  api_base_url={ api_base_url }
                        refresh_ro_fields={['item_cnt','pkg_cnt',]}
                    />;
    let tag_edit_form = <TagContentEditForm  ref={refs.tag_edit_form}  form_id={form_id} />;
    return (
        <>
          <div className="content">
          <div className="container-xl">
              <div className="row">
              <div className="col-xl-10">
              <div className="card">
                <div className="card-header">
                  <div className="d-flex">
                    <button className="btn btn-primary btn-pill" data-toggle="modal"
                        data-addnode="true" data-addchild="false"  data-target={"#"+ form_id}
                        onClick={_render_editform_modal.bind(refs.tag_edit_form)} >
                      add root tag
                    </button>
                    <button className="btn btn-primary btn-pill ml-auto" data-removenode="true"
                        data-removeall="true" onClick={_update_tree_status.bind(refs.hier_data)} >
                        remove all tags in current view
                    </button>
                    <button className="btn btn-primary btn-pill ml-auto" onClick={_save_tree_status.bind(refs.hier_data)} >
                        Save
                    </button>
                    <button className="btn btn-primary btn-pill ml-auto" data-depth="0"
                        data-api_url={ api_base_url.singular_desc.format("root") }
                        onClick={_refresh_root_nodes.bind(refs.hier_data)} >
                        Refresh root nodes
                    </button>
                    <Link className="btn btn-primary btn-pill ml-auto" to='/tags/import'> import from file </Link>
                    <Link className="btn btn-primary btn-pill ml-auto" to='/tags/export'> export to file </Link>
                    <Link className="btn btn-primary btn-pill ml-auto" to='/tags/search'> advanced search </Link>
                  </div>
                </div>
                <div className="card-body">
                    <h3 className="card-title">Hierarchical Tags applied to all saleable items</h3>
                    { hier_data }
                    { tag_edit_form }
                </div>
              </div>
              </div>
              </div>
          </div>
          </div>
        </>
    );
};

export default Tags;

