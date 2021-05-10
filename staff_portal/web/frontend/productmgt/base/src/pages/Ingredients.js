import React from 'react';

import {toggle_visual_elm_showup, _instant_search} from '../js/common/native.js';
import {BaseExtensibleForm} from '../js/common/react.js';
import {AttrKeyValuePairs} from '../components/AttrKeyValuePairs.js'

let api_base_url = {
    plural: '/product/ingredients',
    singular: '/product/ingredient/{0}',
};
let refs = {form_items: React.createRef()};


function _save_items(evt) {
    let form_ref = this.current;
    let succeed_cb = (data, res, props) => {
        let extra   = res.req_args.extra;
        let addlist = extra.addlist;
        // addlist.map((item, idx) => {
        //     let saved_state_item = item.refs.saved_state_item;
        //     saved_state_item['name'] = item.refs['name'].current.value;
        //     return item;
        // });
        //this.setState({saved: this.current.state.saved});
    };
    let callbacks = {succeed: [succeed_cb.bind(form_ref)]};
    let kwargs = {urlpath:api_base_url.plural , callbacks:callbacks,
        fetch_fields:["id", "attributes"],
    };
    form_ref.save(kwargs);
}

function _refresh_items(evt) {
    let form_ref = this.current;
    let valid_field_names = [form_ref._app_item_id_label,
        'type', 'value', ...form_ref._valid_fields_name ];
    let kwargs = {api_url: api_base_url.plural, ordering: ['-id', 'unknown_field_name'],
        page_size: 10, valid_field_names:valid_field_names };
    form_ref.refresh(kwargs);
}

function _delete_items(evt) {
    let form_ref = this.current;
    form_ref.delete(api_base_url.plural);
}

function _undelete_items(evt) {
    let form_ref = this.current;
    form_ref.undelete(api_base_url.plural);
}

function _instant_search_call_api(keyword, api_url) {
    console.log('_instant_search_call_api not implemented yet');
}

function _new_empty_form(evt) {
    let form_ref = this.current;
    form_ref.new_item(undefined, true);
}

class IngredientItems extends BaseExtensibleForm {
    constructor(props) {
        let _valid_fields_name = ['name', 'category', 'attributes',];
        super(props, _valid_fields_name);
    }

    componentDidMount() {
        // let attributes = [
        //     {id:71,  type:10,  value:"goat",},
        //     {id:75,  type:11,  value:2.711,},
        //     {id:123, type:12,  value:-29,},
        // ];
        // let val = {name:'shrimp ball',  id:51  , category: 2, attributes:attributes};
        // this.new_item(val, true);
    }

    _normalize_fn_category(value) {
        return parseInt(value);
    }
    
    new_item(val, update_state) {
        var item = BaseExtensibleForm.prototype.new_item.call(
                this, val, update_state);
        item.refs._chkbox =  React.createRef();
        if(!item.name) {
            item.name = "<new ingredient item>";
        }
        return item;
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


    _load_single_item(evt) {
        let target = evt.nativeEvent.target;
        let menu_classname = "dropdown-menu";
        let dom_showup_classname = "show";
        toggle_visual_elm_showup(target.parentNode, menu_classname, dom_showup_classname);
        // TODO, send GET request to API endpoint
    }
    
    _close_single_item_menu(evt) {
        let menu_classname = "dropdown-menu";
        let dom_showup_classname = "show";
        let target = evt.nativeEvent.target.parentNode;
        while(true) {
            let class_list = Array.from(target.classList);
            if(class_list.indexOf(menu_classname) >= 0) {
                target.classList.remove(dom_showup_classname);
                break;
            } else {
                target = target.parentNode ;
            }
        }
    }

    _single_item_render(val, idx) {
        return (
            <div className="list-item dropdown">
                <input type="checkbox" ref={val.refs._chkbox} className="form-check-input" />
                <a className="nav-link dropdown-toggle" href="#" data-ingre_id={ val.id } 
                    onClick={ this._load_single_item.bind(this) }>
                    {val.name}
                </a>
                <div className="dropdown-menu ">
                    <label className="dropdown-item form-label">
                        Name
                        <input type="text" className="form-control" name="example-text-input"
                            ref={val.refs.name} defaultValue={val.name} />
                    </label>
                    <label className="dropdown-item form-label">
                        Category
                        <select className="form-select" defaultValue={val.category} ref={val.refs.category}>
                            <option value=""> --- select ingredient type --- </option>
                            <option value="1">raw material   </option>
                            <option value="2">work in progress</option>
                            <option value="3">finished goods </option>
                            <option value="4">consumables    </option>
                            <option value="5">equipments     </option>
                        </select>
                    </label>
                    <label className="dropdown-item form-label">
                        Attributes
                        <AttrKeyValuePairs defaultValue={val.attributes} ref={val.refs.attributes} />
                    </label>
                    <div className="row">
                        <div className="col-8">
                        </div>
                        <div className="col-4">
                            <button className="btn btn-primary" > Refresh </button>
                            <button className="btn btn-primary" onClick={ this._close_single_item_menu }>
                                Hide
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        );
    } // end of _single_item_render()
} // end of IngredientItems


const Ingredients = (props) => {
    let _ingre_items = <IngredientItems ref={refs.form_items} />
    let search_bound_obj = {search_api_fn: _instant_search_call_api};
    return (
        <>
          <div className="content">
          <div className="container-xl">
              <div className="row">
              <div className="col-xl-12">
              <p>
                Ingredients/Materials commonly used among all saleable items fabricated by user companies
              </p>
              <div className="card">
                <div className="card-header">
                  <div className="d-flex">
                      <button className="btn btn-primary btn-pill ml-auto" onClick={ _new_empty_form.bind(refs.form_items) } >
                          new empty form
                      </button>
                      <button className="btn btn-primary btn-pill ml-auto" onClick={ _save_items.bind(refs.form_items) } >
                          Save
                      </button>
                      <button className="btn btn-primary btn-pill ml-auto" onClick={ _refresh_items.bind(refs.form_items) } >
                          Refresh
                      </button>
                      <button className="btn btn-primary btn-pill ml-auto" onClick={ _delete_items.bind(refs.form_items) } >
                          Delete
                      </button>
                      <button className="btn btn-primary btn-pill ml-auto" onClick={ _undelete_items.bind(refs.form_items) } >
                          Undelete
                      </button>
                      <div className="mb-3 input-icon" id="searchbar">
                        <input type="text" className="form-control col-l" placeholder="Search..."
                             data-api_url={api_base_url.plural}
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
                <div className="list list-row col-xl-8">
                    { _ingre_items }
                </div>
              </div>
              </div>
              </div>
          </div>
          </div>
        </>
    );
}; // end of  Ingredients

export default Ingredients;

