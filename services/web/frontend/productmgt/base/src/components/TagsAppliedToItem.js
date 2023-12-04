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
        let _valid_fields_name = ['tag_id'];
        super(props, _valid_fields_name);
        this._dom_ref =  React.createRef();
    }

    differ() { // overwrite parent class method
        let dom_elm = this._dom_ref.current;
        let origin = JSON.parse(dom_elm.defaultValue);
        let modify = JSON.parse(dom_elm.value);
        let reduce_fn = (item) => ({tag_id: item.tag_id});
        origin = origin.map(reduce_fn).map((x) => ({origin:x}));
        modify = modify.map(reduce_fn).map((x) => ({modify:x}));
        let longer  = origin.length >  modify.length ? origin: modify;
        let shorter = origin.length <= modify.length ? origin: modify;
        longer = longer.map((item, idx) => {
            let item2 = shorter[idx];
            if(item2) {
                item = {...item, ...item2};
            }
            return item;
        });
        return longer;
    }

    _new_empty_form(evt) {
        this.new_item(undefined, true);
    }

    _remove_form(evt) {
        this.comp.remove_item(this.val, true);
    }

    componentDidMount() {
        let dom_elm = this._dom_ref.current;
        let construct_option = (item) => ({tag_id: item.id, value:item.name});
        let whitelist = _tags_option.results.map(construct_option);
        // let tagged_opts = this.state.saved.map(construct_option);
        let saved_ids = this.state.saved.map((item) => (item.tag_id));
        let tagged_opts = whitelist.filter((item) => (saved_ids.includes(item.tag_id)));
        dom_elm.defaultValue = JSON.stringify(tagged_opts);
        let dropdown_cfg = {maxItems:20, classname:'tags-look', enabled:0, closeOnSelect:false};
        let props = {className:'tagify--custom-dropdown', dropdown: dropdown_cfg, whitelist:whitelist};
        dom_elm._tagify = new Tagify(dom_elm, props);
    }

    render() {
        return <input ref={this._dom_ref} />;
    } // end of render()
} // end of  class TagsAppliedToItem

