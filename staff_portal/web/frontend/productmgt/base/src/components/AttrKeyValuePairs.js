import React from 'react';

import {BaseExtensibleForm} from '../js/common/react.js';
import {AttrTypeItems} from '../pages/AttrTypes.js';


// For select and option DOM elements , it is a good practice to explicitly give
// every single one of them an unique key , that can avoid the select elements
// from behaving unexpectedly. The select elements without given unique key will
// reset all its chosen options on re-rendering ...
let element_unique_key_increment = 0;
let attr_types = {results:null, inited:false};

function _load_attr_types() {
    let attype_comp = new AttrTypeItems({});
    let callbacks_succeed = (data, res, props) => {
        attr_types.results = data;
        attr_types.inited = true;
        ////let saved_cp = [...this.state.saved];
        ////this.setState({attr_types: attr_types.results, saved:[], });
        ////this.setState({saved:saved_cp,});
        ////this.setState({attr_types: this.state.attr_types});
    };
    let callbacks = {succeed : [callbacks_succeed],};
    let kwargs = {api_url: '/product/attrtypes', callbacks:callbacks};
    attype_comp.refresh(kwargs);
} 

_load_attr_types();

export class AttrKeyValuePairs extends BaseExtensibleForm {
    constructor(props) {
        let _valid_fields_name = ['type', 'value',];
        super(props, _valid_fields_name);
    }

    _normalize_fn_type(value) {
        return parseInt(value);
    }
    
    _normalize_fn_value(value) {
        return String(value);
    }

     _new_empty_form(evt) {
        this.new_item(undefined, true);
    }
     
     _remove_form(evt) {
        this.comp.remove_item(this.val, true);
     }
    
    _single_item_render(val, idx) {
        let attr_type_options = attr_types.results.map((item) => {
            //console.log("this.state.attr_types name:"+ item.name +" , id:"+ item.id);
            element_unique_key_increment += 1;
            return <option key={element_unique_key_increment} value={item.id}>{item.name}</option>;
        });
        element_unique_key_increment += 1;
        let default_option = <option key={element_unique_key_increment} value=""> --- select attribute type --- </option>;
        attr_type_options.splice(0, 0, default_option);
        let bound_remove_obj = {comp:this, val:val};
        element_unique_key_increment += 1;
        let select_obj = <select className="form-select" defaultValue={val.type}
                        ref={val.refs.type} key={element_unique_key_increment} > {attr_type_options} </select> ;
        return (
            <div className="row">
                <div className="col-6">
                    { select_obj }
                </div>
                <div className="col-4">
                    <input type="text" className="form-control" ref={val.refs.value}
                        defaultValue={val.value} />
                </div>
                <div className="col-2">
                    <button className="btn btn-primary" onClick={this._remove_form.bind(bound_remove_obj)}>
                        remove
                    </button>
                </div>
            </div>
        );
    } // end of _single_item_render()

    render() {
        return (
            <div className="content" id="dynamic_form_wrapper">
                <div className="row">
                    <div className="col-4">
                        <button className="btn btn-primary" onClick={this._new_empty_form.bind(this)}>
                            Add new attribute
                        </button>
                    </div>
                </div>
                <div className="container-xl">
                    { this.state.saved.map(this._single_item_render_wrapper.bind(this)) }
                </div>
            </div>
        );
    } // end of render()
} // end of IngredientItems   


