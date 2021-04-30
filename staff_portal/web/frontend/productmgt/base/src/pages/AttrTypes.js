import React from 'react';

import {clear_array, _instant_search} from '../js/common/native.js';
import {BaseExtensibleForm} from '../js/common/react.js';


let api_base_url = '/product/attrtypes';
let refs = {form_items: React.createRef()};


function _save_attr_types(evt) {
    let form_ref = this.current;
    form_ref.save(api_base_url);
}

function _refresh_items(evt) {
    let form_ref = this.current;
    let kwargs = {api_url: api_base_url, ordering: ['-id', 'dtype'],
        page_size: 13 };
    form_ref.refresh(kwargs);
}


function _delete_attr_types(evt) {
    let form_ref = this.current;
    form_ref.delete(api_base_url);
}

function _undelete_attr_types(evt) {
    let form_ref = this.current;
    form_ref.undelete(api_base_url);
}

function _instant_search_call_api(keyword, api_url) {
    console.log('not implemented yet');
}

function _new_empty_form(evt) {
    let form_ref = this.current;
    form_ref.new_item(undefined, true);
}


class AttrTypeItems extends BaseExtensibleForm {
    constructor(props) {
        super(props);
        this._valid_fields_name = ['name', 'dtype'];
    }
    
    componentDidMount() {
        //let val = {name:'diaameter',    id:51  , dtype: 4,};
        //{name:'diameter',    id:51  , dtype:'attr_val_float',},
        //{name:'length (cm)', id:620 , dtype:'attr_val_int',  },
        //{name:'color',       id:13  , dtype:'attr_val_str',  },
        //this.new_item(val, true);
    }

    _normalize_fn_dtype(value) {
        return parseInt(value);
    }
    
    delete(api_base_url) {
        var delete_list = [];
        this.state.saved.map((val, idx) => {
            if(val.refs._chkbox.current.checked) {
                delete_list.push(val);
            }
            return val;
        });
        var valid_field_names = [this._app_item_id_label,];
        BaseExtensibleForm.prototype.delete.call(
            this, {
            api_url:api_base_url, varlist:delete_list, 
            valid_field_names:valid_field_names,
        });
    }

    undelete(api_base_url) {
        var valid_field_names = [this._app_item_id_label, ...this._valid_fields_name];
        BaseExtensibleForm.prototype.undelete.call(
            this, {api_url:api_base_url,
            valid_field_names:valid_field_names,});
    }

    new_item(val, update_state) {
        var item = BaseExtensibleForm.prototype.new_item.call(
                this, val, update_state);
        item.refs._chkbox =  React.createRef();
        return item;
    }

    _single_item_render(val, idx) {
        return (
            <div className="input-group">
              <input type="checkbox" ref={val.refs._chkbox} className="form-check-input" />
              <input type="text" ref={val.refs.name} defaultValue={val.name} className="form-control" />
              <select className="form-select" defaultValue={val.dtype} ref={val.refs.dtype} >
                  <option value=""> ---- select data type ---- </option>
                  <option value="1">string</option>
                  <option value="2">integer</option>
                  <option value="3">positive integer</option>
                  <option value="4">float</option>
              </select>
            </div>
        );
    }
} // end of AttrTypeItems


const AttrTypes = (props) => {
    let _attr_type_items = <AttrTypeItems ref={refs.form_items} />
    let search_bound_obj = {search_api_fn: _instant_search_call_api};
    return (
        <>
          <div className="content">
          <div className="container-xl">
              <div className="row">
              <div className="col-xl-12">
              <p>
                attribute types commonly used among all saleable items
              </p>
              <div className="card">
                <div className="card-header">
                  <div className="d-flex">
                      <button className="btn btn-primary btn-pill ml-auto" onClick={ _new_empty_form.bind(refs.form_items) } >
                          new empty form
                      </button>
                      <button className="btn btn-primary btn-pill ml-auto" onClick={ _save_attr_types.bind(refs.form_items) } >
                          Save
                      </button>
                      <button className="btn btn-primary btn-pill ml-auto" onClick={ _refresh_items.bind(refs.form_items) } >
                          Refresh
                      </button>
                      <button className="btn btn-primary btn-pill ml-auto" onClick={ _delete_attr_types.bind(refs.form_items) } >
                          Delete
                      </button>
                      <button className="btn btn-primary btn-pill ml-auto" onClick={ _undelete_attr_types.bind(refs.form_items) } >
                          Undelete
                      </button>
                      <div className="mb-3 input-icon" id="searchbar">
                        <input type="text" className="form-control col-l" placeholder="Search..."
                             data-api_url={api_base_url}
                             onKeyUp={_instant_search.bind(search_bound_obj)} />
                        <span className="input-icon-addon"  onClick={ _instant_search.bind(search_bound_obj) }>
                            <svg xmlns="http://www.w3.org/2000/svg" className="icon" width="24" height="24" viewBox="0 0 24 24" strokeWidth="2" stroke="currentColor" fill="none" strokeLinecap="round" strokeLinejoin="round">
                                <path stroke="none" d="M0 0h24v24H0z"/>
                                <circle cx="10" cy="10" r="7" />
                                <line x1="21" y1="21" x2="15" y2="15" />
                            </svg>
                        </span>
                      </div>
                  </div>
                </div>
                <div className="card-body">
                    <div className="row col-xl-8">
                        { _attr_type_items }
                    </div>
                </div>
              </div>
              </div>
              </div>
          </div>
          </div>
        </>
    );
};

export default AttrTypes;

