import * as toolkit from "/static/js/toolkit.js";

class QuotaForm extends toolkit.ReactBaseForm {
    constructor(props) {
        super(props);
        this._default_fields = [
            {name:'max_num_email',      label:'max # emails',      type:'number', value:'0'},
            {name:'max_num_phone',      label:'max # phones',      type:'number', value:'0'},
            {name:'max_num_addr',       label:'max # addresses',   type:'number', value:'0'},
            {name:'max_bookings',       label:'max # bookings',          type:'number', value:'0'},
            {name:'max_entry_waitlist', label:'max # entry in waitlist', type:'number', value:'0'},
        ];
    }

    _new_single_form(init_form_data) {
        var _components = [];
        var _fields = this._get_all_fields_prop(this._default_fields, init_form_data);
        const tag_input = 'input';
        const tag_div   = 'div';
        var labels = Object.keys(_fields);
        var td_per_tr = [ [], [], [] ];
        var tr = [];
        var tbody = null;

        td_per_tr[0].push(React.createElement('td', {key: this.get_unique_key()},));
        td_per_tr[1].push(React.createElement('td', {key: this.get_unique_key()}, "customize"));
        td_per_tr[2].push(React.createElement('td', {key: this.get_unique_key()}, "default"));
        for(var idx = 0; idx < labels.length; idx++) {
            td_per_tr[0].push(React.createElement('td', {key: this.get_unique_key()}, _fields[labels[idx]].label));
            var edit_comp = React.createElement(tag_input, _fields[labels[idx]]);
            td_per_tr[1].push(React.createElement('td', {key: this.get_unique_key()}, edit_comp));
            var status_comp = React.createElement(tag_div, {key: this.get_unique_key(), id: "status_" + labels[idx], children:'0'} );
            td_per_tr[2].push(React.createElement('td', {key: this.get_unique_key()}, status_comp));
        }
        tr.push(React.createElement('tr', {key: this.get_unique_key(), children: td_per_tr[0]})) ;
        tr.push(React.createElement('tr', {key: this.get_unique_key(), children: td_per_tr[1]})) ;
        tr.push(React.createElement('tr', {key: this.get_unique_key(), children: td_per_tr[2]})) ;
        tbody = React.createElement('tbody', {key: this.get_unique_key(), children: tr});
        _components.push(React.createElement('table', {border:1, key: this.get_unique_key(), children: [tbody],}));
        _components.push(React.createElement('br', {key: this.get_unique_key()}));
        return _components;
    }

    get_formdata() {
        var form = this.form_ref.current;
        var dfs  = this._default_fields;
        for(var idx = 0; idx < dfs.length; idx++) {
            var obj = dfs[idx].ref.current;
            var edit_value = parseInt(obj.value);
            // use default value if any of following conditions happens...
            if(isNaN(edit_value) || edit_value == 0) {
                 var default_value = parseInt(form.querySelector("div[id=status_"+ obj.name +"]").innerText);
                 console.log(" .... detect NaN or zero in edit value, replace it with default_value : "+ default_value );
                 obj.value = default_value;
            }
            //// console.log(" .... quota field name: "+ obj.name +" , value: "+ obj.value +" , origin edit_value : "+ isNaN(edit_value) );
        }
        return toolkit.ReactBaseForm.prototype.get_formdata.call(this); 
    }
} // end of QuotaForm


function get_default_quota_form_props()
{
    var out = {key:0, form:{}};
    out.form['max_num_email']      = { className:'short_content_field'};
    out.form['max_num_phone']      = { className:'short_content_field'};
    out.form['max_num_addr']       = { className:'short_content_field'};
    out.form['max_bookings']       = { className:'short_content_field'};
    out.form['max_entry_waitlist'] = { className:'short_content_field'};
    return out;
}


function update_quota_on_add_tag(form, new_tag)
{
    var quota = { max_num_email      : {obj: form.querySelector("div[id=status_max_num_email]")     , new_value:0},
                  max_num_phone      : {obj: form.querySelector("div[id=status_max_num_phone]")     , new_value:0} ,
                  max_num_addr       : {obj: form.querySelector("div[id=status_max_num_addr]")      , new_value:0} , 
                  max_bookings       : {obj: form.querySelector("div[id=status_max_bookings]")      , new_value:0} ,
                  max_entry_waitlist : {obj: form.querySelector("div[id=status_max_entry_waitlist]"), new_value:0},
                };
    quota.max_num_email.new_value      = Math.max(new_tag.max_num_email      , parseInt(quota.max_num_email.obj.innerText)     );
    quota.max_num_phone.new_value      = Math.max(new_tag.max_num_phone      , parseInt(quota.max_num_phone.obj.innerText)     );
    quota.max_num_addr.new_value       = Math.max(new_tag.max_num_addr       , parseInt(quota.max_num_addr.obj.innerText)      );
    quota.max_bookings.new_value       = Math.max(new_tag.max_bookings       , parseInt(quota.max_bookings.obj.innerText)      );
    quota.max_entry_waitlist.new_value = Math.max(new_tag.max_entry_waitlist , parseInt(quota.max_entry_waitlist.obj.innerText));

    quota.max_num_email.obj.innerText      = quota.max_num_email.new_value     ;
    quota.max_num_phone.obj.innerText      = quota.max_num_phone.new_value     ;
    quota.max_num_addr.obj.innerText       = quota.max_num_addr.new_value      ;
    quota.max_bookings.obj.innerText       = quota.max_bookings.new_value      ;
    quota.max_entry_waitlist.obj.innerText = quota.max_entry_waitlist.new_value;
}


function update_quota_on_del_tag(form, tag_list)
{
    var quota = { max_num_email      : {obj: form.querySelector("div[id=status_max_num_email]")     , new_value:0},
                  max_num_phone      : {obj: form.querySelector("div[id=status_max_num_phone]")     , new_value:0} ,
                  max_num_addr       : {obj: form.querySelector("div[id=status_max_num_addr]")      , new_value:0} , 
                  max_bookings       : {obj: form.querySelector("div[id=status_max_bookings]")      , new_value:0} ,
                  max_entry_waitlist : {obj: form.querySelector("div[id=status_max_entry_waitlist]"), new_value:0},
                };
    for (var idx = 0; idx < tag_list.length; idx++) {
        quota.max_num_email.new_value      = Math.max(tag_list[idx].max_num_email      , quota.max_num_email.new_value     );
        quota.max_num_phone.new_value      = Math.max(tag_list[idx].max_num_phone      , quota.max_num_phone.new_value     );
        quota.max_num_addr.new_value       = Math.max(tag_list[idx].max_num_addr       , quota.max_num_addr.new_value      );
        quota.max_bookings.new_value       = Math.max(tag_list[idx].max_bookings       , quota.max_bookings.new_value      );
        quota.max_entry_waitlist.new_value = Math.max(tag_list[idx].max_entry_waitlist , quota.max_entry_waitlist.new_value);
    }
    quota.max_num_email.obj.innerText      = quota.max_num_email.new_value     ;
    quota.max_num_phone.obj.innerText      = quota.max_num_phone.new_value     ;
    quota.max_num_addr.obj.innerText       = quota.max_num_addr.new_value      ;
    quota.max_bookings.obj.innerText       = quota.max_bookings.new_value      ;
    quota.max_entry_waitlist.obj.innerText = quota.max_entry_waitlist.new_value;
}

export {QuotaForm, get_default_quota_form_props, update_quota_on_add_tag, update_quota_on_del_tag };

