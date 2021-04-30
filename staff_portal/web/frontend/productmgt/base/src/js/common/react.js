import React from 'react';

import {BaseUrl, DEFAULT_API_REQUEST_OPTIONS} from '../constants.js';
import {get_input_dom_value, get_select_dom_value, set_input_dom_value, set_select_dom_value, clear_array, clear_props_object, APIconsumer,} from './native.js';
// Note , this file cannot be linked outside react application scope
// but this file is commonly used among severl react applications, how to deal with the issue ?


export class BaseModalForm extends React.Component {
    constructor(props) {
        super(props);
        this.state = {};
        this._user_form_ref = React.createRef();
        this._input_refs = {};
    }
        
    get_field_names() {
        return Object.keys(this._input_refs);
    }

    get_data() {
        // retrieve input data from user form in object format
        var out = {}; // {title: 'electronic sensor', subtitle:'DIY maker',};
        for(var key in this._input_refs) {
            var ref = this._input_refs[key].current;
            if(ref instanceof HTMLInputElement) {
                out[key] = get_input_dom_value(ref);
            } else if(ref instanceof HTMLSelectElement) {
                out[key] = get_select_dom_value(ref);
            } else if(ref instanceof React.Component) {
                throw Error("not implemented , to get data from react component");
            }
        }
        return out;
    }

    set_data(values) {
        for(var key in this._input_refs) {
            var value = values[key];
            var ref = this._input_refs[key].current;
            if(ref instanceof HTMLInputElement) {
                set_input_dom_value(ref, value);
            } else if(ref instanceof HTMLSelectElement) {
                set_select_dom_value(ref, value);
            } else if(ref instanceof React.Component) {
                throw Error("not implemented , to set data to react component");
            }
        }
    }

    render(elements) {
        return (
            <div className={elements.class_name.modal.frame.join(' ')}  id={this.props.form_id} tabIndex="-1" role="dialog" aria-hidden="true">
              <div className="modal-dialog modal-lg modal-dialog-centered" role="document">
                <div className="modal-content">
                  <div className={elements.class_name.modal.header.join(' ')}>
                    <h5 className={elements.class_name.modal.title.join(' ')}> {elements.form_title} </h5>
                    <button type="button" className="close" data-dismiss="modal" aria-label="Close">
                      <svg xmlns="http://www.w3.org/2000/svg" className="icon" width="24" height="24" viewBox="0 0 24 24" strokeWidth="2" stroke="currentColor" fill="none" strokeLinecap="round" strokeLinejoin="round"><path stroke="none" d="M0 0h24v24H0z"/><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
                    </button>
                  </div>
                  <div className={elements.class_name.modal.body.join(' ')} ref={ this._user_form_ref } >
                    { elements.user_form }
                  </div>
                  <div className={elements.class_name.modal.footer.join(' ')}>
                    <button className={elements.class_name.modal.btn_cancel.join(' ')} data-dismiss="modal">
                      Cancel
                    </button>
                    <button className={elements.class_name.modal.btn_apply.join(' ')} data-dismiss="modal" onClick={this.state.callback_submit}>
                      <svg xmlns="http://www.w3.org/2000/svg" className="icon" width="24" height="24" viewBox="0 0 24 24" strokeWidth="2" stroke="currentColor" fill="none" strokeLinecap="round" strokeLinejoin="round"><path stroke="none" d="M0 0h24v24H0z"/><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
                      { elements.submit_btn_label }
                    </button>
                  </div>
                </div>
              </div>
            </div>
        );
    }
}; // end of class BaseModalForm


export class BaseExtensibleForm extends React.Component {
    constructor(props) {
        super(props);
        this._valid_fields_name = []; // has to be updated by subclasses
        this.state = {saved: [], };
        this._uncommitted_items = {added:[], edited:{}, removed:{},};
        this._app_item_id_label = props._app_item_id_label ? props._app_item_id_label: 'id';
        var default_api_props = {num_retry: 4, wait_interval_ms: 305};
        var api_props = props.api ? props.api: default_api_props;
        this.api_caller = new APIconsumer(api_props);
    }
    
    _gather_unsaved_item(val, idx) {
        var origin = {};
        var modify = {};
        this._valid_fields_name.map((key) => {
            let normalize_fn = this['_normalize_fn_'+ key];
            if(normalize_fn) {
                origin[key] = normalize_fn(val[key]);
                modify[key] = normalize_fn(val.refs[key].current.value);
            } else {
                origin[key] = val[key];
                modify[key] = val.refs[key].current.value;
            }
            return key;
        });
        var serialized_origin = JSON.stringify(origin);
        var serialized_modify = JSON.stringify(modify);
        var _id = val[this._app_item_id_label];
        if(serialized_origin !== serialized_modify) {
            if(_id) {
                modify[this._app_item_id_label] = _id;
                this._uncommitted_items.edited[_id] = modify;
            } else {
                this._uncommitted_items.added.push(modify);
            }
        }
    } // end of _gather_unsaved_item

    save(urlpath, callbacks) {
        var skipped = true;
        clear_array(this._uncommitted_items.added);
        clear_props_object(this._uncommitted_items.edited);
        this.state.saved.map(this._gather_unsaved_item.bind(this));
        // append internal callback to the end of callback list for clearing
        //  uncommitted items when the API endpoint is successfully called
        var callbacks_copied = APIconsumer.copy_callbacks(callbacks, ['succeed']);
        callbacks_copied.succeed.push(this._commit_callback_succeed.bind(this));

        var url = BaseUrl.API_HOST + urlpath ;
        var req_opt = this._prepare_save_api_req();
        // Note POST and PUT request will be sent concurrently , for extra check
        // between PUT and POST requests, application developers should overwrite
        // this method
        if(req_opt.post.body) {
            this.api_caller.start({base_url:url, req_opt:req_opt.post, callbacks:callbacks_copied,
                params:{fields:[this._app_item_id_label]},});
            skipped = false;
        }
        if(req_opt.put.body) {
            this.api_caller.start({base_url:url, req_opt:req_opt.put, callbacks:callbacks_copied,});
            skipped = false;
        }
        return skipped;
    } // end of save()
    
    _prepare_save_api_req() {
        // make 2 seperate API calls for added nodes and edited nodes
        var req_opt = {
            post: DEFAULT_API_REQUEST_OPTIONS.POST(),
            put : DEFAULT_API_REQUEST_OPTIONS.PUT(),
        };
        var valid_field_names = [this._app_item_id_label, ...this._valid_fields_name];
        req_opt.post.body = APIconsumer.serialize(this._uncommitted_items.added,  valid_field_names);
        req_opt.put.body  = APIconsumer.serialize(this._uncommitted_items.edited, valid_field_names);
        return req_opt;
    } // end of _prepare_save_api_req
    

    _commit_callback_succeed(data, res, props) {
        var req_args = res.req_args;
        var req_mthd = req_args.req_opt.method ;
        if(req_mthd === 'POST') {
            this._uncommitted_items.added.map((val, idx) => {
                val[this._app_item_id_label] = data[idx][this._app_item_id_label];
                return val;
            });
            clear_array(this._uncommitted_items.added);
        } else if(req_mthd === 'DELETE') {
            var varlist = req_args.extra.varlist;
            varlist.map((val, idx) => {
                this.remove_item(val, false);
                return val; 
            });
            //this.state.saved.map((val) => {
            //    var _id    = val[this._app_item_id_label];
            //    var _name  = val.name;
            //    console.log("after delete, state.saved, _id:"+ _id +" , _name:"+ _name);
            //    return val;
            //});
            let tmp_shallow_clone = [...this.state.saved];
            clear_array(this.state.saved);
            this.setState({saved: this.state.saved});
            this.state.saved.push(...tmp_shallow_clone);
            this.setState({saved: this.state.saved});
        } else if(req_mthd === 'PATCH') {
            let datalist = data.results ? data.results: data;
            datalist.map((val, idx) => {
                this.new_item(val, false);
            });
            this.setState({saved: this.state.saved});
        }
    } // end of _commit_callback_succeed()


    refresh(kwargs) {
        var api_url = kwargs.api_url;
        if(api_url && api_url.length > 0) {
            // get valid field names from the form, then add extra read-only fields at backend
            var valid_field_names = null;
            if (kwargs.valid_field_names) {
                valid_field_names = kwargs.valid_field_names;
            } else {
                valid_field_names = [this._app_item_id_label, ...this._valid_fields_name];
            }
            let query_params = {fields: valid_field_names,}
            if(kwargs.ordering) {
                query_params.ordering = kwargs.ordering.join(',');
            }
            if(kwargs.page_size) {
                query_params.page_size = kwargs.page_size;
                query_params.page = kwargs.page ? kwargs.page: 1;
            }
            let req_opt    = DEFAULT_API_REQUEST_OPTIONS.GET();
            let callbacks  = null;
            if(kwargs.callbacks) { // do partial copy, then add extra callback for internal use
                callbacks = APIconsumer.copy_callbacks(kwargs.callbacks, ['succeed']);
                callbacks.succeed.push(this._refresh_callback_succeed.bind(this));
            } else {
                callbacks = {succeed : [this._refresh_callback_succeed.bind(this)],};
            }
            let extra = kwargs.extra;
            this.api_caller.start({base_url: BaseUrl.API_HOST + api_url, req_opt:req_opt,
                callbacks:callbacks,  params: query_params, extra:extra});
        } else {
            console.log("no URL for the API");
        }
    } // end of refresh()

    _refresh_callback_succeed(data, res, props) {
        if (!data || (data && data.result === undefined && data.length === 0) ||
                (data && data.result !== undefined && data.results.length === 0)) {
            console.log('succeed callback on refresh, but nothing retrieved');
            return;
        }
        // TODO: make use of other fields in data e.g. data.count, data.previous
        // data.next when pagination is enabled in the API call.
        if(res.req_args.params.page > 1) {
        } else {
            clear_array(this.state.saved);
            this.setState({saved: this.state.saved});
        }
        let datalist = data.results ? data.results: data;
        datalist.map((val, idx) => {
            this.new_item(val, false);
        });
        this.setState({saved: this.state.saved});
    }
    
    delete(kwargs) {
        var urlpath = kwargs.api_url;
        var varlist = kwargs.varlist;
        if(varlist && varlist.length > 0 && urlpath) {
            var callbacks = null;
            if(kwargs.callbacks) {
                callbacks = APIconsumer.copy_callbacks(kwargs.callbacks, ['succeed']);
            } else {
                callbacks = {succeed:[]};
            }
            callbacks.succeed.push(this._commit_callback_succeed.bind(this));
            var url = BaseUrl.API_HOST + urlpath ;
            var valid_field_names = kwargs.valid_field_names ? kwargs.valid_field_names: [this._app_item_id_label,];
            var req_opt = DEFAULT_API_REQUEST_OPTIONS.DELETE()
            req_opt.body = APIconsumer.serialize(varlist,  valid_field_names);
            this.api_caller.start({base_url:url, req_opt:req_opt, callbacks:callbacks,
                 extra:{varlist: varlist}});
        } else {
            console.log("no URL or no item selected to delete");
        }
    } // end of delete()

    undelete(kwargs) {
        var urlpath = kwargs.api_url;
        if(urlpath) {
            var callbacks = null;
            if(kwargs.callbacks) {
                callbacks = APIconsumer.copy_callbacks(kwargs.callbacks, ['succeed']);
            } else {
                callbacks = {succeed:[]};
            }
            callbacks.succeed.push(this._commit_callback_succeed.bind(this));
            var url = BaseUrl.API_HOST + urlpath ;
            var valid_field_names = kwargs.valid_field_names ? kwargs.valid_field_names: [this._app_item_id_label,];
            let query_params = {fields: valid_field_names,}
            var req_opt = DEFAULT_API_REQUEST_OPTIONS.PATCH(); // req_opt.body = '';
            this.api_caller.start({base_url:url, req_opt:req_opt, callbacks:callbacks,
                 params: query_params});
        } else {
            console.log("no URL or no item selected to un-delete");
        }
    } // end of undelete()
    
    new_item(val, update_state) {
        let saved = this.state.saved ;
        let _new_state_item = {refs:{}};
        if(val && val[this._app_item_id_label]) {
            _new_state_item[this._app_item_id_label] = val[this._app_item_id_label];
        }
        this._valid_fields_name.map((key) => {
            if(val && val[key]) {
                _new_state_item[key] = val[key];
            }
            _new_state_item.refs[key] = React.createRef();
            return key;
        });
        saved.push(_new_state_item);
        if(update_state) {
            this.setState({saved: saved});
        }
        return _new_state_item;
    } // end of new_item()

    remove_item(val, update_state) {
        let eq = (item) => {
            var del_id  = val[this._app_item_id_label];
            var item_id = item[this._app_item_id_label];
            return del_id !== item_id;
        };
        let after_delete = this.state.saved.filter(eq);
        // TRICKY , you're only allowed to send the same object reference
        // of the state field, tell react.js that the `saved` field
        // should be cleaned up and it will be re-rendered later on
        clear_array(this.state.saved);
        if(update_state) {
            this.setState({saved: this.state.saved});
        }
        this.state.saved.push(...after_delete);
        if(update_state) {
            this.setState({saved: this.state.saved});
        }
    } // end of remove_item()

    _single_item_render(val, idx) {
        throw "not implemented yet";
    }

    render() {
        return (
            <div className="row" id="dynamic_form_wrapper">
            { this.state.saved.map(this._single_item_render) }
            </div>
        );
    }
}; // end of class BaseExtensibleForm


