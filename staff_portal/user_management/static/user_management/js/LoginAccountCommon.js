import * as toolkit from "/static/js/toolkit.js";


class LoginAccountForm extends toolkit.ReactBaseForm {
    constructor(props) {
        super(props);
        this._default_fields = [
            // TODO, add captcha to slow down brute-force attack (e.g. bot/scrapy to perform enumeration)
            // only used for error report, find better way to do this
            {name:'activate_token', label:'activate token',   type:'text',  value:''},
            {name:'old_uname',   label:'old username',  type:'text',      value:''},
            {name:'old_passwd',  label:'old password',  type:'password',  value:''},
            {name:'username',    label:'username',      type:'text',      value:''},
            {name:'password',    label:'password',      type:'password',  value:''},
            {name:'password2',   label:'confirm password',  type:'password',   value:''},
        ];
    }

    _new_single_form(init_form_data) {
        var _fields = this._get_all_fields_prop(this._default_fields, init_form_data);
        var field_grps = [];
        // render enabled fields, ignore those disabled
        var field_names = Object.keys(_fields).filter((x) => (x != "activate_token"));
        for(var idx = 0; idx < field_names.length; idx++) {
            var _field = _fields[field_names[idx]];
            if(_field.disabled != undefined && _field.disabled == false) {
                field_grps.push(
                    React.createElement('div', {key: this.get_unique_key(), className:'form-group',},
                        React.createElement('label', {key: this.get_unique_key()}, _field.label),
                        React.createElement('input', _field),
                    ),
                );
            }
        }
        var card_header = React.createElement('div', {key: this.get_unique_key(), className:'card-header', children: [],});
        var card_body   = React.createElement('div', {key: this.get_unique_key(), className:'card-body',   children: field_grps,});
        return [card_header, card_body];
    }
} // end of class LoginAccountForm


export function get_default_form_props(container) {
    var out = {};
    out.container = container;
    out.className = "card card-primary p-1 bg-light";
    out.btns = {addform: {enable:false, evt_cb: append_new_form},};
    out.enable_close_btn = false;
    out.form = {};
    out.form['rand_denied_field'] = 98;
    out.form['username']   = {className:"form-control", disabled:false,  placeholder:"8-20 chars, e.g. A-Z, a-z, 0-9" };
    out.form['password']   = {className:"form-control", disabled:false,  placeholder:"at least 12 chars, e.g. A-Z, a-z, 0-9" };
    out.form['password2']  = {className:"form-control", disabled:false,  placeholder:"repeat new password" };
    out.form['old_uname']  = {className:"form-control", disabled:false, };
    out.form['old_passwd'] = {className:"form-control", disabled:false, };
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
        LoginAccountForm.add_form(container, props[idx]);
    } // end of for loop
} // end of append_new_form


