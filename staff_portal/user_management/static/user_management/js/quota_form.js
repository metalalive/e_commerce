import * as toolkit from "/static/js/toolkit.js";

var valid_quotatype_data = null;
var usagetype_seeker = null;

class QuotaForm extends toolkit.ReactBaseForm {
    constructor(props) {
        super(props);
        this._default_fields = [
            {name:'id',          type:'hidden', value:''},
            {name:'maxnum',      type:'number', label:'max num', value:''},
            {name:'usage_type',  type:'text'  , label:'usage type', },
        ];
    }


    _new_single_form(init_form_data) {
        var _fields = this._get_all_fields_prop(this._default_fields, init_form_data);
        var _components = [];
        _components.push(
            React.createElement("input", _fields['id']),
            React.createElement('div', {key: this.get_unique_key(), className:'col-5'},
                React.createElement(toolkit.TagInstantSearchBox, _fields['usage_type'])
            ),
            React.createElement('div', {key: this.get_unique_key(), className:'col-2'},
                "max. num"
            ),
            React.createElement('div', {key: this.get_unique_key(), className:'col-2'},
                React.createElement("input", _fields['maxnum'])
            ),
            React.createElement('div', {key: this.get_unique_key(), className:'col-3',
                children:toolkit.render_form_window_btns(this)},),
        );
        var row = React.createElement("div", {key: this.get_unique_key(), className:'row p-0',children:_components}, )
        return [row];
    } // end of _new_single_form
} // end of QuotaForm


function get_default_form_props(dom_ref) {
    var out = toolkit.get_default_form_props(dom_ref, append_new_form);
    var dropdown = {maxItems:40, classname:'tags-look', enabled:0, closeOnSelect:false};
    out.btns.addform.className   = "btn btn-sm btn-outline-primary p-1";
    out.btns.closeform.className = "btn btn-sm btn-outline-primary p-1";
    out.form['rand_denied_field'] = 98;
    out.form['id'] = {};
    out.form['maxnum'] = {className:"form-control", defaultValue:'1' };
    out.form['usage_type'] = { defaultValue: [],  whitelist: valid_quotatype_data, extract_prop_on_submit:'id', 
                 className:'tagify--custom-dropdown', dropdown: dropdown, placeholder:'choose quota usage type',
                 mode: 'select'};
    return out;
}


function append_new_form(dom_ref, props) {
    if(props == null){
        props = [];
    } else if (!(props instanceof Array)) {
        throw "initial data to render quota subforms must be a list of property objects"
    }
    if(props.length == 0) {
        props[0] = get_default_form_props(dom_ref);
    }
    var container = (dom_ref instanceof HTMLElement) ? dom_ref: dom_ref.current;
    for(var idx = 0; idx < props.length; idx++) {
        QuotaForm.add_form(container, props[idx]);
    }
} // end of append_new_form


function load_api_quotatype_cb(avail_data, props)
{
    if((props.http_resp_status/100) >= 4) {
        return;
    }
    valid_quotatype_data = (avail_data.results) ? avail_data.results: avail_data;
    valid_quotatype_data = valid_quotatype_data.map((x) => {return {id:x.id, value:x.label};});
    usagetype_seeker = toolkit.cvt_list_to_dict(valid_quotatype_data, 'id');
}


function render_with_data(data) {
    var out = [];
    for (var jdx = 0; jdx < data.length; jdx++) {
        var prop = get_default_form_props(null);
        prop.form['id'].defaultValue     = data[jdx].id;
        prop.form['maxnum'].defaultValue = data[jdx].maxnum;
        data[jdx].usage_type.value = usagetype_seeker[ data[jdx].usage_type.id ].value;
        prop.form['usage_type'].defaultValue = JSON.stringify( [data[jdx].usage_type] );
        out.push(prop);
    } // end of inner loop
    return out;
}

export {
    append_new_form, get_default_form_props, load_api_quotatype_cb, usagetype_seeker,
    valid_quotatype_data, render_with_data
};

