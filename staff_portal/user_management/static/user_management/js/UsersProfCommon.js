import * as toolkit        from "/static/js/toolkit.js";
import * as quotaform      from "./quota_form.js";
import * as locationform  from "/static/location/js/location_form.js";
import * as emailform     from "./email_form.js";
import * as phoneform     from "./phone_form.js";
import * as usrgrp        from "./UserGroupsCommon.js"


class UserProfileForm extends toolkit.ReactBaseForm {
    constructor(props) {
        super(props);
        this._default_fields = [
            {name:'id',          label:'ID',                    type:'hidden', value:''},
            {name:'first_name',  label:'First name',            type:'text',   value:''},
            {name:'last_name',   label:'Last name',             type:'text',   value:''},
            {name:'enable_auth', label:'Enable login account',  type:'checkbox',   value:'yes'},
            {name:'groups',      label:'Groups',          value:[],},
            {name:'roles',       label:'Extra roles for the user',  value:[],},
            {name:'quota',       label:'Override quota for the user', value:[],},
            {name:'emails',      label:'Emails',          value:[],},
            {name:'phones',      label:'Phones',          value:[],},
            {name:'locations',   label:'Geo Locations',   value:[],},
        ];
    }

    _render_name_field(_fields, field_grps) {
        var name_field = [];
        name_field.push(
            React.createElement('div', {key: this.get_unique_key(), className:'form-group col-6',},
                React.createElement('label', {key: this.get_unique_key()}, _fields['first_name'].label),
                React.createElement('input', _fields['first_name']),
            )
        );
        name_field.push(
            React.createElement('div', {key: this.get_unique_key(), className:'form-group col-6',},
                React.createElement('label', {key: this.get_unique_key()}, _fields['last_name'].label),
                React.createElement('input', _fields['last_name']),
            )
        );
        field_grps.push( React.createElement('div', {key: this.get_unique_key(), className:'row',  children: name_field,}) );
    }

    _render_roles_field(_field, field_grps) {
        var _components = [
            React.createElement('label', {key: this.get_unique_key()}, _field.label),
            React.createElement(toolkit.TagInstantSearchBox, _field),
        ];
        field_grps.push( React.createElement('div', {key: this.get_unique_key(),
            className:'form-group',  children: _components,}));
    }

    _render_subform_container(_field, pkg, field_grps) {
        var kwargs = {subform_name:_field.name, field_in_prop: _field, out: field_grps};
        kwargs.fn = {addform: pkg.append_new_form};
        this.render_generic_subform_container(kwargs);
    }

    _new_single_form(init_form_data) {
        var field_grps = [];
        var _fields = this._get_all_fields_prop(this._default_fields, init_form_data);

        var tool_btns = toolkit.render_form_window_btns(this);
        var form_title = React.createElement('label', {key: this.get_unique_key(), className:'card-title'}, 
                "Form #"+ this.props.container.children.length);
        var field_grps = [];

        field_grps.push( React.createElement('input', _fields['id']) );
        this._render_name_field(_fields, field_grps);
        this._render_roles_field(_fields['groups'], field_grps);
        this._render_roles_field(_fields['roles'], field_grps);
        this._render_subform_container(_fields['quota'], quotaform, field_grps)
        this._render_subform_container(_fields['emails'], emailform, field_grps);
        this._render_subform_container(_fields['phones'], phoneform, field_grps);
        this._render_subform_container(_fields['locations'], locationform, field_grps);

        var header_tool = React.createElement('div', {key: this.get_unique_key(), className:'card-tools',  children: [tool_btns],});
        var card_header = React.createElement('div', {key: this.get_unique_key(), className:'card-header', children: [form_title, header_tool],});
        var card_body   = React.createElement('div', {key: this.get_unique_key(), className:'card-body',   children: field_grps,});
        return [card_header, card_body];
    } // end of _new_single_form
} // end of class UserProfileForm


export function get_default_form_props(container) {
    var out = toolkit.get_default_form_props(container, append_new_form);
    var dropdown = {maxItems:40, classname:'tags-look', enabled:0, closeOnSelect:false};
    out.className = "card card-primary p-0 bg-light";
    out.btns.addform.className   = "btn btn-primary";
    out.btns.closeform.className = "btn btn-primary";
    out.form['rand_denied_field'] = 98;
    out.form['id'] = {};
    out.form['first_name'] = {className:"form-control", placeholder:"Enter first name" };
    out.form['last_name']  = {className:"form-control", placeholder:"Enter last name" };
    out.form['groups'] = {defaultValue:[],  whitelist: usrgrp.get_valid_usrgrp_data(), dropdown: dropdown,
                          className:'tagify--custom-dropdown', placeholder:'choose groups for the user',
                          evt_cb_add: null, evt_cb_remove:null, maxTags: 5, extract_prop_on_submit:'id', };
    out.form['roles'] = {defaultValue: [], whitelist: usrgrp.get_valid_role_data(), extract_prop_on_submit:'id', 
                         className:'tagify--custom-dropdown', dropdown: dropdown, placeholder:'choose roles',};
    out.form['quota'] = {defaultValue: null};
    out.form['emails'] = {defaultValue: null};
    out.form['phones'] = {defaultValue: null};
    out.form['locations'] = {defaultValue: null};
    return out;
}

export function append_new_form(container, props) {
    if(props == null){ props = [];}
    if((props instanceof Array) && (props.length == 0)) {
        props[0] = get_default_form_props(container) ;
    }
    // init field data for each form
    for(var idx = 0; idx < props.length; idx++) {
        props[idx].key = container.children.length;
        UserProfileForm.add_form(container, props[idx]);
    } // end of for loop
} // end of append_new_form


export function load_api_data(form_layout, edit_data, uris) {
    // append functions to check whether API data loading is done
    if(!form_layout.chk_load_api_done_cbs) {
        form_layout.chk_load_api_done_cbs = [];
    }
    form_layout.chk_load_api_done_cbs.push(locationform.init_api_data_done);
    form_layout.get_default_form_props = get_default_form_props;
    form_layout.append_new_form = append_new_form;
    usrgrp.load_api_data(form_layout, uris);
    // load location data
    locationform.load_country_data(form_layout, edit_data);
}


