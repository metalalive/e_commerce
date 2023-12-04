import React from 'react';
import {toggle_visual_elm_showup} from '../js/common/native.js';
import {BaseExtensibleForm} from '../js/common/react.js';
import {AttrKeyValuePairs} from '../components/AttrKeyValuePairs.js'

export class CommonPresentableProductItemForm extends BaseExtensibleForm {
    constructor(props, _valid_fields_name) {
        if(_valid_fields_name === undefined || _valid_fields_name === null) {
            _valid_fields_name = [];
        }
        _valid_fields_name.push('name', 'attributes');
        super(props, _valid_fields_name);
    }

    new_item(val, update_state) {
        var item = BaseExtensibleForm.prototype.new_item.call(
                this, val, update_state);
        item.refs._chkbox =  React.createRef();
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
        let toggleable_menu = this._single_item_menu_render(val, idx);
        return (
            <div className="list-item dropdown">
                <input type="checkbox" ref={val.refs._chkbox} className="form-check-input" />
                <a className="nav-link dropdown-toggle" href="#" data-ingre_id={ val.id } 
                    onClick={ this._load_single_item.bind(this) }>
                    {val.name}
                </a>
                <div className="dropdown-menu ">
                    { toggleable_menu }
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

    _single_item_menu_render(val, idx) {
        let errmsg =  "subclasses should overwrite CommonPresentableProductItemForm._single_item_menu_render to provide menu layout in their application.";
        throw new Error(errmsg);
    }

    _single_item_name_field_render(val, idx) {
        return (
            <label className="form-label">
                Name
                <input type="text" className="form-control" name="example-text-input"
                    ref={val.refs.name} defaultValue={val.name} />
            </label>
        );
    }

    _single_item_attributes_field_render(val, idx, extra_amount) {
        let extra_fields_name = null;
        if (extra_amount) {
            extra_fields_name = ["extra_amount"];
        }
        return (
            <label className="form-label">
                Attributes
                <AttrKeyValuePairs defaultValue={val.attributes}  ref={val.refs.attributes}
                    extra_fields_name={extra_fields_name} />
            </label>
        );
    }
}; // end of class CommonPresentableProductItemForm

