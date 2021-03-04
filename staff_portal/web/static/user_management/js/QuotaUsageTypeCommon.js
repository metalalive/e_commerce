import * as toolkit from "/static/js/toolkit.js";


class QuotaUsageTypeForm extends toolkit.ReactBaseForm {
    constructor(props) {
        super(props);
        this._default_fields = [
            {name:'id',      label:'ID',  type:'hidden', value:''},
            {name:'label',   label:'Description',    type:'text',  value:''},
            {name:'appname', label:'Application',       type:'text',  value:''},
            {name:'material',label:'Model Class Type',  type:'text',  value:''},
        ];
    }

    _new_single_form(init_form_data) {
        var _fields = this._get_all_fields_prop(this._default_fields, init_form_data);
        var tool_btns = toolkit.render_form_window_btns(this);
        var material = React.createElement(toolkit.TagInstantSearchBox, _fields['material']);
        _fields['appname'].nxt_lvl_elm = material;
        var appname  = React.createElement(toolkit.TagInstantSearchBox, _fields['appname']);
        var name_field  = [
            React.createElement('input', _fields['id']),
            React.createElement('label', {key: this.get_unique_key()}, "Resource Type"),
            React.createElement('div', {key: this.get_unique_key(), className:'row'},
                React.createElement('div', {key: this.get_unique_key(), className:'col-5'}, _fields['appname'].label),
                React.createElement('div', {key: this.get_unique_key(), className:'col-7'}, _fields['material'].label),
            ),
            React.createElement('div', {key: this.get_unique_key(), className:'row'},
                React.createElement('div', {key: this.get_unique_key(), className:'col-5'}, appname,),
                React.createElement('div', {key: this.get_unique_key(), className:'col-7'}, material,),
            ),
            React.createElement('label', {key: this.get_unique_key()}, _fields['label'].label),
            React.createElement('input', _fields['label']),
        ];
        var field_grps = [];
        field_grps.push( React.createElement('div', {key: this.get_unique_key(), className:'form-group',  children: name_field,}) );
        var header_tool = React.createElement('div', {key: this.get_unique_key(), className:'card-tools',  children: [tool_btns],});
        var card_header = React.createElement('div', {key: this.get_unique_key(), className:'card-header', children: [header_tool],});
        var card_body   = React.createElement('div', {key: this.get_unique_key(), className:'card-body',   children: field_grps,});
        return [card_header, card_body];
    }
} // end of RoleForm


function load_material_list(evt) {
    var new_tag = evt.detail.value.data;
    var comp = evt.detail.tagify.react_comp;
    var new_whitelist = comp.props.app_models_list[new_tag.value];
    if(new_whitelist.length > 0 && new_whitelist[0].model) {
        new_whitelist.map((x) => { x.value = x.model; delete x.model; return x;});
    }
    comp.update_nxt_lvl_comp(new_whitelist);
}


function unload_material_list(evt) {
    var comp = evt.detail.tagify.react_comp;
    comp.update_nxt_lvl_comp([]);
}


export function get_default_form_props(container) {
    var dropdown = {maxItems:40, classname:'tags-look', enabled:0, closeOnSelect:false};
    var out = toolkit.get_default_form_props(container, append_new_form);
    out.btns.addform.className   = "btn  btn-primary";
    out.btns.closeform.className = "btn  btn-primary";
    out.container = container;
    out.className = "card card-primary p-0 bg-light";
    out.form['rand_denied_field'] = 98;
    out.form['id'] = {};
    out.form['label'] = {className:"form-control", placeholder:"Extra description for the resource type" };
    out.form['appname'] = {defaultValue: [], whitelist: container.apps_list, extract_prop_on_submit:null, 
             className:'tagify--custom-dropdown', dropdown: dropdown, placeholder:'choose application',
             evt_cb_add: load_material_list, evt_cb_remove: unload_material_list, mode: 'select',
             app_models_list:container.app_models_list, };
    out.form['material'] = {defaultValue: [], whitelist: [], extract_prop_on_submit:'id', 
             className:'tagify--custom-dropdown', dropdown: dropdown, placeholder:'choose model class as resource type', mode: 'select',};
    return out;
}


export function append_new_form(container, props) {
    if(props == null){
        props = [];
    } else if (!(props instanceof Array)) {
        throw "initial data to render quota subforms must be a list of property objects"
    }
    if(props.length == 0) {
        props[0] = get_default_form_props(container);
    }
    // init field data for each form
    for(var idx = 0; idx < props.length; idx++) {
        props[idx].key = container.children.length;
        QuotaUsageTypeForm.add_form(container, props[idx]);
    } // end of for loop
} // end of append_new_form


