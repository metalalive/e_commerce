import * as toolkit from "/static/js/toolkit.js";
import * as quotaform from "./quota_form.js";

var valid_usrgrp_data   = null;
export var usrgrp_seeker = null;

var valid_role_data   = null;
export var role_seeker = null;



class UserGroupForm extends toolkit.ReactBaseForm {
    constructor(props) {
        super(props);
        this._default_fields = [
            {name:'id',      label:'ID',     type:'hidden', value:''},
            {name:'name',    label:'name',   type:'text',   value:''},
            {name:'exist_parent',  label:'existing parent', },
            {name:'new_parent',    label:'new parent', },
            {name:'roles',         label:'roles',       value:''},
            {name:'quota', label:'quota', },
        ];
    }

    componentDidMount() {
        toolkit.ReactBaseForm.prototype.componentDidMount.call(this);
        // update new parent list in previous generated forms
        var container = this.props.container;
        var new_parent_list = Array.from(container.querySelectorAll("select[name=new_parent]"));
        for(var idx = 0; idx < new_parent_list.length ; idx++) {
            var opt_elm = document.createElement("option");
            opt_elm.text = "form #"+ container.children.length;
            opt_elm.value = container.children.length;
            new_parent_list[idx].appendChild(opt_elm);
        }
    } // end of componentDidMount


    _render_parent_field(_fields, field_grps) {
        var _children = [];
        _children.push(React.createElement('label', {key: this.get_unique_key()}, "Hierarchy, choose parent group"));
        _children.push(React.createElement('div', {key: this.get_unique_key(), className:'row'}, 
                    React.createElement('div', {key: this.get_unique_key(), className:'col-9'},
                        React.createElement('span', {key: this.get_unique_key(), className:'text-muted'},_fields['exist_parent'].label)
                    ),
                    React.createElement('div', {key: this.get_unique_key(), className:'col-3'},
                        React.createElement('span', {key: this.get_unique_key(), className:'text-muted',
                            hidden:_fields['new_parent'].hidden}, _fields['new_parent'].label)
                    ),
                ));
        _children.push(React.createElement('div', {key: this.get_unique_key(), className:'row'}, 
                    React.createElement('div', {key: this.get_unique_key(), className:'col-9'},
                        React.createElement(toolkit.TagInstantSearchBox, _fields['exist_parent']) ),
                    React.createElement('div', {key: this.get_unique_key(), className:'col-3'},
                        React.createElement("select", _fields['new_parent'],
                           React.createElement(toolkit.ChoiceFieldRender, {key: this.get_unique_key(), options: _fields['new_parent'].opts})
                        )
                    ),
                ));
        field_grps.push( React.createElement('div', {key: this.get_unique_key(), className:'form-group',  children: _children,}) );
    } // end of _render_parent_field


    _render_roles_field(_field, field_grps) {
        var _children = [];
        _children.push(React.createElement('label', {key: this.get_unique_key()}, _field.label));
        _children.push(React.createElement(toolkit.TagInstantSearchBox, _field));
        field_grps.push( React.createElement('div', {key: this.get_unique_key(), className:'form-group',  children: _children,}) );
    } // end of _render_roles_field


    _render_quota_subforms(_field, field_grps)
    {
        var kwargs = {subform_name:'quota', field_in_prop: _field, out: field_grps};
        kwargs.fn = {addform:quotaform.append_new_form};
        this.render_generic_subform_container(kwargs);
    }

    _new_single_form(init_form_data) {
        var _fields = this._get_all_fields_prop(this._default_fields, init_form_data);
        var form_title = React.createElement('label', {key: this.get_unique_key(), className:'card-title'}, 
                "Form #"+ this.props.container.children.length);
        var tool_btns = toolkit.render_form_window_btns(this);
        var name_field  = [];
        name_field.push(React.createElement('label', {key: this.get_unique_key()}, _fields['name'].label));
        name_field.push(React.createElement('input', _fields['name']));
        name_field.push(React.createElement('input', _fields['id']));
        var field_grps = [];
        field_grps.push( React.createElement('div', {key: this.get_unique_key(), className:'form-group',  children: name_field,}) );
        this._render_parent_field(  _fields, field_grps);
        this._render_roles_field(   _fields['roles'], field_grps);
        this._render_quota_subforms(_fields['quota'], field_grps);
        var header_tool = React.createElement('div', {key: this.get_unique_key(), className:'card-tools',  children: [tool_btns],});
        var card_header = React.createElement('div', {key: this.get_unique_key(), className:'card-header', children: [form_title, header_tool],});
        var card_body   = React.createElement('div', {key: this.get_unique_key(), className:'card-body',   children: field_grps,});
        return [card_header, card_body];
    }
} // end of RoleForm


export function perform_api_call(props)
{
    var api_handler = new toolkit.APIconsumer({api_base_url: props.url, req_opt: props.req_opt, query_params: props.query_params,
                           finish_cbs: props.callbacks,  caller:props.caller});
    api_handler.start( props.query_params );
    return api_handler;
}



export function get_valid_usrgrp_data() {
    return valid_usrgrp_data;
}

export function get_valid_role_data() {
    return valid_role_data;
}


function hier_compare_fn(a,b) {
    var out = 0;
    if(a.depth > b.depth) {
        out = 1;
    } else if(a.depth < b.depth) {
        out = -1;
    }
    return out;
}


function gen_usrgrp_hierarchy_label(item) {
    var data = item.ancestors;
    data = data.sort(hier_compare_fn).reverse();
    item.value = data.map(x => x.ancestor.name).join('/') +'/'+ item.name;
    //// item.value = data.map(x => usrgrp_seeker[x.ancestor.id].name ).join('/') +'/'+ item.name;
    //// delete item.name;
    return item;
}


export function load_api_usrgrp_cb(avail_data, props)
{
    if((props.http_resp_status/100) >= 4) {
        return;
    }
    valid_usrgrp_data = (avail_data.results) ? avail_data.results: avail_data;
    usrgrp_seeker = toolkit.cvt_list_to_dict(valid_usrgrp_data, 'id');
    valid_usrgrp_data = valid_usrgrp_data.map(gen_usrgrp_hierarchy_label);
}


export function load_api_role_cb(avail_data, props)
{
    if((props.http_resp_status/100) >= 4) {
        return;
    }
    valid_role_data = (avail_data.results) ? avail_data.results: avail_data;
    valid_role_data = valid_role_data.map((x) => {x.value = x.name; return x;});
    role_seeker =  toolkit.cvt_list_to_dict(valid_role_data, 'id');
}


export function get_default_form_props(container) {
    var out = toolkit.get_default_form_props(container, append_new_form);
    var new_grp_opts = [{label:"------", value:""}, ];
    for(var idx = 0; idx < container.children.length; idx++) {
        new_grp_opts.push({label:"form #"+ idx, value:idx});
    }
    var dropdown = {maxItems:40, classname:'tags-look', enabled:0, closeOnSelect:false};
    out.className = "card card-primary p-0 bg-light";
    out.btns.addform.className   = "btn btn-primary";
    out.btns.closeform.className = "btn btn-primary";
    out.form['rand_denied_field'] = 98;
    out.form['id'] = {};
    out.form['name'] = {className:"form-control", placeholder:"Enter group name" };
    out.form['roles'] = {defaultValue: [], label:'Roles', whitelist: valid_role_data, extract_prop_on_submit:'id', 
                 className:'tagify--custom-dropdown', dropdown: dropdown, placeholder:'choose roles',};
    out.form['exist_parent'] = {defaultValue: [], label:'from existing group', whitelist: valid_usrgrp_data,
                 extract_prop_on_submit:'id', className:'tagify--custom-dropdown', placeholder:'choose parent group',
                 dropdown: dropdown, mode: 'select'};
    out.form['new_parent'] = {className:"form-control", label:'from new group', hidden:false, opts:new_grp_opts};
    out.form['quota'] = {defaultValue: null};
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
        UserGroupForm.add_form(container, props[idx]);
    } // end of for loop
} // end of append_new_form



function init_api_data_done()
{
    return (valid_usrgrp_data && valid_role_data && quotaform.valid_quotatype_data) ? true: false ;
}


export function load_api_data(form_layout, uris) {
    const req_opt = {method:"GET", headers:{'accept':'application/json'},};
    var api_props = null;
    // append functions to check whether API data loading is done
    if(!form_layout.chk_load_api_done_cbs) {
        form_layout.chk_load_api_done_cbs = [];
    }
    form_layout.chk_load_api_done_cbs.push(init_api_data_done);
    // append function to render data to visual component
    if(!form_layout.get_default_form_props) {
        form_layout.get_default_form_props = get_default_form_props;
    }
    if(!form_layout.append_new_form) {
        form_layout.append_new_form = append_new_form;
    }

    // API, load all roles
    api_props = {url: uris.authrole, req_opt:req_opt, callbacks: [load_api_role_cb, toolkit.api_data_ready_cb],
                 query_params: {fields:'id,name'}, caller:form_layout }; // ,permissions
    perform_api_call(api_props);
    // API, load all quota usage types,
    api_props = {url: uris.quotatype, req_opt:req_opt, callbacks: [quotaform.load_api_quotatype_cb, toolkit.api_data_ready_cb],
                 query_params: {fields:'id,label'}, caller:form_layout };
    perform_api_call(api_props);
    // load existing user groups data
    api_props = {url: uris.usrgrp, req_opt:req_opt, callbacks: [load_api_usrgrp_cb, toolkit.api_data_ready_cb],
                 query_params: {fields:'id,name,ancestors,depth,ancestor'}, caller:form_layout };
    // roles,max_num_addr,max_num_phone,max_num_email,max_bookings,max_entry_waitlist
    perform_api_call(api_props);
} // end of load_api_data


