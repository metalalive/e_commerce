import React from 'react';
import Tagify from '@yaireo/tagify';
import {BaseUrl} from '../js/constants.js';
import {BaseExtensibleForm} from '../js/common/react.js';
// import {} from '../pages/Tags.js';

let _tags_option = {results:[], inited:false};

export function load_tags_option() {
    let _valid_fields_name = ['name',];
    let props = {_app_item_id_label: 'id'};
    let base_comp = new BaseExtensibleForm(props, _valid_fields_name);
    let callbacks_succeed = (data, res, props) => {
        _tags_option.results = data;
        _tags_option.inited = true;
    };
    let callbacks = {succeed : [callbacks_succeed],};
    let kwargs = {api_url: '/product/tags', callbacks:callbacks,
            urlhost:BaseUrl.API_HOST };
    base_comp.refresh(kwargs);
}

// TODO, sync once the tags has changed in the other page
load_tags_option();


export class TagsAppliedToItem extends BaseExtensibleForm {
    constructor(props) {
        let _valid_fields_name = ['name',];
        super(props, _valid_fields_name);
        this._dom_ref =  React.createRef();
    }
    _normalize_fn_name(value) {
        return String(value);
    }
    _new_empty_form(evt) {
        this.new_item(undefined, true);
    }
    _remove_form(evt) {
        this.comp.remove_item(this.val, true);
    }

    componentDidMount() {
        let dom_elm = this._dom_ref.current;
        let construct_option = (item) => ({id: item.id, value:item.name});
        let tagged_opts = this.state.saved.map(construct_option);
        let whitelist = _tags_option.results.map(construct_option);
        let dropdown_cfg = {maxItems:20, classname:'tags-look', enabled:0, closeOnSelect:false};
        let props = {className:'tagify--custom-dropdown', dropdown: dropdown_cfg,
            defaultValue: tagged_opts, whitelist:whitelist};
        let _tagify = new Tagify(dom_elm, props);
    }

    render() {
        return <input ref={this._dom_ref} />;
    } // end of render()
} // end of  class TagsAppliedToItem

