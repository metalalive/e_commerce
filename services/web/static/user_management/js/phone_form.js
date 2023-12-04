import * as toolkit from "/static/js/toolkit.js";


export class PhoneForm extends toolkit.ReactBaseForm {
    constructor(props) {
        super(props);
        this._default_fields = [
            {name:'id',    type:'hidden', value:''}, // id specific to the phone number
            {name:'uid',   type:'hidden', value:''}, // id bound with specific user, used only in update scene
            {name:'country_code',  type:'text', value:'', label:'country code'}, // TODO: max size
            {name:'line_number',   type:'text', value:'', label:'line number'},
        ];
    }

    _new_single_form(init_form_data) {
        var _fields = this._get_all_fields_prop(this._default_fields, init_form_data);
        var _components = [
            React.createElement('input', _fields['id']),
            React.createElement('input', _fields['uid']),
            React.createElement('div', {key: this.get_unique_key(), className:'col-2'},
                React.createElement('input', _fields['country_code'])
            ),
            React.createElement('div', {key: this.get_unique_key(), className:'col-5'},
                React.createElement('input', _fields['line_number'])
            ),
            React.createElement('div', {key: this.get_unique_key(), className:'col-3',
                children:toolkit.render_form_window_btns(this)},),
        ];
        var row = React.createElement("div", {key: this.get_unique_key(), className:'row p-0',children:_components}, )
        return [row];
    }
} // end of class PhoneForm



export function get_default_form_props(dom_ref) {
    var out = toolkit.get_default_form_props(dom_ref, append_new_form);
    var dropdown = {maxItems:40, classname:'tags-look', enabled:0, closeOnSelect:false};
    out.btns.addform.className   = "btn btn-sm btn-outline-primary p-1";
    out.btns.closeform.className = "btn btn-sm btn-outline-primary p-1";
    out.form['id']  = {};
    out.form['uid'] = {};
    out.form['country_code'] = {className:"form-control", defaultValue: '', placeholder:'e.g. 886',};
    out.form['line_number']  = {className:"form-control", defaultValue: '', placeholder:'e.g. 935001001'};
    return out;
}


export function  append_new_form(dom_ref, props) {
    if(props == null){
        props = [];
    } else if (!(props instanceof Array)) {
        throw "initial data to render subforms must be a list of property objects"
    }
    if((props instanceof Array) && (props.length == 0)) {
        props[0] = get_default_form_props(dom_ref);
    }
    var container = (dom_ref instanceof HTMLElement) ? dom_ref: dom_ref.current;
    for(var idx = 0; idx < props.length; idx++) {
        PhoneForm.add_form(container, props[idx]);
    }
}


export function render_with_data(data) {
    var out = [];
    for (var jdx = 0; jdx < data.length; jdx++) {
        var prop = get_default_form_props(null);
        prop.form['uid'].defaultValue  = data[jdx].uid;
        prop.form['id'].defaultValue   = data[jdx].id;
        prop.form['country_code'].defaultValue = data[jdx].country_code;
        prop.form['line_number'].defaultValue = data[jdx].line_number;
        out.push(prop);
    } // end of inner loop
    return out;
}

