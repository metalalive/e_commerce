import React from 'react';
import Tagify from '@yaireo/tagify';

import {BaseUrl} from '../js/constants.js';
import {BaseExtensibleForm} from '../js/common/react.js';
import uom_json from '../data/unit_of_measurement.json';

let element_unique_key_increment = 0;
let _tagify_ingredients_option = {results:[], inited:false};
let uom = {results:[], inited:false};

function init_uom() {
    let entries = Object.entries(uom_json);
    uom.results = entries.map((item, idx) => {
        let value = item[1];
        return {value:value[0], label:value[1]};
    });
    uom.inited = true;
}
init_uom();


export function load_ingredients_option() {
    let _valid_fields_name = ['name',];
    let props = {_app_item_id_label: 'id'};
    let base_comp = new BaseExtensibleForm(props, _valid_fields_name);
    let callbacks_succeed = (data, res, props) => {
        _tagify_ingredients_option.results = data;
        _tagify_ingredients_option.inited = true;
    };
    let callbacks = {succeed : [callbacks_succeed],};
    let kwargs = {api_url: '/product/ingredients', callbacks:callbacks,
            urlhost:BaseUrl.API_HOST };
    base_comp.refresh(kwargs);
}
// TODO, sync once the tags has changed in the other page
load_ingredients_option();


export class IngredientsAppliedToSaleableItem extends BaseExtensibleForm {
    constructor(props) {
        let _valid_fields_name = ['ingredient_id', 'unit', 'quantity'];
        super(props, _valid_fields_name);
        this._container_ref = React.createRef();
        let bound_fn = this._on_observe_item.bind(this);
        this._container_observer = new MutationObserver(bound_fn);
    }

    _normalize_fn_ingredient_id(value, referrer) {
        if(referrer instanceof HTMLInputElement) {
            // grab updated value from tagify instance
            let deserialized = JSON.parse(value);
            if(deserialized[0]) {
                value = deserialized[0].ingredient_id;
            } else {
                value = undefined;
            }
        }
        return value;
    }

    _normalize_fn_unit(value) {
        return parseInt(value);
    }

    _normalize_fn_quantity(value) {
        return parseFloat(value);
    }
     
     _new_empty_form(evt) {
        this.new_item(undefined, true);
    }

    _remove_form(evt) {
        this.comp.remove_item(this.val, true);
    }

    _render_tag_instant_searchbar() {
        // each ingredient is assigned with a tagify instance
        this.state.saved.map((val, idx) => {
            if(val.refs._ingredient_tagify) {
                return;
            }
            let dom_elm = val.refs.ingredient_id.current;
            let construct_option_fn = (item) => ({value:item.name, ingredient_id: item.id});
            let whitelist = _tagify_ingredients_option.results.map(construct_option_fn);
            if(val.ingredient_id) {
                let chosen_ingres =  whitelist.filter((item) => (item.ingredient_id == val.ingredient_id));
                dom_elm.defaultValue = JSON.stringify(chosen_ingres);
            }
            let dropdown_cfg = {maxItems:20, classname:'tags-look', enabled:0, closeOnSelect:false};
            let props = {className:'tagify--custom-dropdown', dropdown: dropdown_cfg,
                    mode:'select',  whitelist:whitelist};
            let _instance = new Tagify(dom_elm, props);
            val.refs._ingredient_tagify = _instance;
        });
    } // end of _render_tag_instant_searchbar()

    _on_observe_item(mutations) {
        // observer invokes this function every time when it detects any change
        // in the DOM element specified by this._container_observer.observe()
        this._render_tag_instant_searchbar();
    }

    componentDidMount() {
        // note this function is invoked only when the react
        // component is mounted at the first time
        let props = {attributes:false, childList:true, characterData:false,
                subtree:true };
        this._container_observer.observe(this._container_ref.current, props);
        this._render_tag_instant_searchbar();
    } // end of componentDidMount()

    _single_item_render(val, idx) {
        let uom_options = [];
        if(uom.results) {
            uom_options = uom.results.map((item) => {
                element_unique_key_increment += 1;
                return <option key={element_unique_key_increment} value={item.value}>{item.label}</option>;
            });
        }
        element_unique_key_increment += 1;
        let unit_select_obj = <select className="form-select" defaultValue={val.unit}
                        ref={val.refs.unit} key={element_unique_key_increment}> {uom_options} </select> ;
        let bound_remove_obj = {comp:this, val:val};
        let ingre_tagsearch = <input ref={val.refs.ingredient_id} />;
        return (
            <div className="row">
                <input type="hidden" className="form-control" ref={val.refs.id} defaultValue={val.id} />
                <div className="col-4">{ ingre_tagsearch }</div>
                <div className="col-2">{ unit_select_obj }</div>
                <div className="col-2">
                    <input type="text" className="form-control" ref={val.refs.quantity}
                        defaultValue={val.quantity} />
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
                            Add new ingredient
                        </button>
                    </div>
                </div>
                <div className="container-xl" ref={this._container_ref}>
                    { this.state.saved.map(this._single_item_render_wrapper.bind(this)) }
                </div>
            </div>
        );
    } // end of render()
} // end of class IngredientsAppliedToSaleableItem

