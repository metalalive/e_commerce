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

function _instant_search(evt) {
    let invoker = evt.nativeEvent.target;
    var keyword = null;
    var api_url = null;
    if(invoker instanceof HTMLInputElement) {
        if (evt.code == "Enter") {
            console.log('start instant search ... keycode:'+ evt.code);
            keyword = invoker.value;
            api_url = invoker.dataset.api_url;
        }
    } else if(invoker instanceof HTMLButtonElement) {
        var txtfield = invoker.parentNode.querySelector("input");
        keyword = txtfield.value;
        api_url = txtfield.dataset.api_url;
    }
    if (keyword && api_url) {
        let tree_ref = this.current;
        tree_ref.search(keyword, api_url);
    }
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
                        search_ro_fields={['item_cnt','pkg_cnt','ancestors']}
                    />;
    let tag_edit_form = <TagContentEditForm  ref={refs.tag_edit_form}  form_id={form_id} />;
    return (
        <>
          <div className="content">
          <div className="container-xl">
              <div className="row">
              <div className="col-xl-12">
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
                  </div>
                </div>
                <div className="card-body">
                    <div className="row">
                        <div className="col-6 col-sm-4 col-md-2 col-xl mb-3">
                            <Link className="btn btn-primary btn-pill ml-auto" to='/tags/sortby'> sort by </Link>
                        </div>
                        <div className="col-6 col-sm-4 col-md-2 col-xl mb-3">
                            <div className="mb-3 input-icon" id="searchbar">
                              <input type="text" className="form-control col-l" placeholder="Search..."
                                   onKeyUp={ _instant_search.bind(refs.hier_data) }
                                   data-api_url={api_base_url.plural} />
                              <span className="input-icon-addon"  onClick={ _instant_search.bind(refs.hier_data) }>
                                  <svg xmlns="http://www.w3.org/2000/svg" className="icon" width="24" height="24" viewBox="0 0 24 24" strokeWidth="2" stroke="currentColor" fill="none" strokeLinecap="round" strokeLinejoin="round">
                                      <path stroke="none" d="M0 0h24v24H0z"/>
                                      <circle cx="10" cy="10" r="7" />
                                      <line x1="21" y1="21" x2="15" y2="15" />
                                  </svg>
                              </span>
                            </div>
                        </div>
                    </div>
                    <div className="row">
                        <h3 className="card-title">Hierarchical Tags applied to all saleable items</h3>
                    </div>
                    <div className="row">
                        { hier_data }
                    </div>
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

