import * as toolkit from "/static/js/toolkit.js";

var valid_permission_data = null;


class RoleForm extends toolkit.ReactBaseForm {
    constructor(props) {
        super(props);
        this._default_fields = [
            {name:'id',      label:'ID',     type:'hidden', value:''},
            {name:'name',    label:'name',   type:'text',   value:''},
            {name:'permissions',  label:'permissions',  value:[]},
        ];
    }

    _new_single_form(init_form_data) {
        var _fields = this._get_all_fields_prop(this._default_fields, init_form_data);
        var tool_btns = [];
        this.template_visual_comp = document.getElementById('template_visual_comp');
        if(this.props.btns.addform.enable) {
            this.btn_props.add_form = {className:"btn btn-primary", children:["add form"], title:'add one more form',
                 onClick: this.props.btns.addform.evt_cb.bind(this, this.props.container, null),
                 icon_id:'add_form_icon', icon_tag:'svg', } ;
            tool_btns.push(this._generate_button(this.btn_props.add_form));
        }
        this.btn_props.close_form = {className:"btn btn-primary", title:"close the form", icon_id:'close_form_icon', icon_tag:'svg', };
        tool_btns.push(this._generate_close_button("close", this.btn_props.close_form));
        var name_field  = [];
        name_field.push(React.createElement('label', {key: this.get_unique_key()}, _fields['name'].label));
        name_field.push(React.createElement('input', _fields['name']));
        name_field.push(React.createElement('input', _fields['id']));
        var permis_field = [];
        permis_field.push(React.createElement('label', {key: this.get_unique_key()}, _fields['permissions'].label));
        permis_field.push(React.createElement(toolkit.TagInstantSearchBox, _fields['permissions']));
        var field_grps = [];
        field_grps.push( React.createElement('div', {key: this.get_unique_key(), className:'form-group',  children: name_field,}) );
        field_grps.push( React.createElement('div', {key: this.get_unique_key(), className:'form-group',  children: permis_field,}) );
        var header_tool = React.createElement('div', {key: this.get_unique_key(), className:'card-tools',  children: [tool_btns],});
        var card_header = React.createElement('div', {key: this.get_unique_key(), className:'card-header', children: [header_tool],});
        var card_body   = React.createElement('div', {key: this.get_unique_key(), className:'card-body',   children: field_grps,});
        return [card_header, card_body];
    }
} // end of RoleForm


export function get_default_form_props(container) {
    var out = {};
    var dropdown = {maxItems:40, classname:'tags-look', enabled:0, closeOnSelect:false};
    out.container = container;
    out.className = "card card-primary p-0 bg-light";
    out.btns = {addform: {enable:true, evt_cb: append_new_form},};
    out.enable_close_btn = true;
    out.form = {};
    out.form['rand_denied_field'] = 98;
    out.form['id'] = {};
    out.form['name'] = {className:"form-control", placeholder:"Enter role label" };
    out.form['permissions'] = { defaultValue: [],  whitelist: valid_permission_data, extract_prop_on_submit:'id', 
                 className:'tagify--custom-dropdown', dropdown: dropdown, placeholder:'choose permissions for the role',};
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
        RoleForm.add_form(container, props[idx]);
    } // end of for loop
} // end of append_new_form


export function load_api_permission_cb(avail_data, props)
{
    valid_permission_data = (avail_data.results) ? avail_data.results: avail_data;
    valid_permission_data = valid_permission_data.map((x) => {return {id:x.id, value:x.name};});
}



