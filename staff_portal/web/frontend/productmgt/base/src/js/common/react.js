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
    constructor(props, _valid_fields_name) {
        super(props);
        this._valid_fields_name = _valid_fields_name ; // []; // has to be updated by subclasses
        this.state = {saved: undefined,};
        this.state.saved = [];
        this._uncommitted_items = {added:[], edited:{}, removed:{},};
        this._app_item_id_label = props._app_item_id_label ? props._app_item_id_label: 'id';
        var default_api_props = {num_retry: 4, wait_interval_ms: 305};
        var api_props = props.api ? props.api: default_api_props;
        this.api_caller = new APIconsumer(api_props);
        this._unique_key_increment = 1;
        if(props.defaultValue) {
            props.defaultValue.map((val) => {
                this.new_item(val, true);
                return val;
            });
        }
    }

    _diff_iter(val, idx) {
        var _id = val[this._app_item_id_label];
        let origin = {};
        var modify = {refs: val.refs};
        if(_id) {
            origin[this._app_item_id_label] = _id;
            modify[this._app_item_id_label] = _id;
        }
        this._valid_fields_name.map((key) => {
            let elm = val.refs[key].current;
            if((elm instanceof HTMLInputElement) ||
                (elm instanceof HTMLSelectElement))
            {
                let normalize_fn = this['_normalize_fn_'+ key];
                if(normalize_fn) {
                    origin[key] = normalize_fn(val[key]);
                    modify[key] = normalize_fn(elm.value);
                } else {
                    origin[key] = val[key];
                    modify[key] = elm.value;
                }
            } else if(elm instanceof BaseExtensibleForm){
                var difference = elm.differ();
                origin[key] = difference.map((item) => (item.origin));
                modify[key] = difference.map((item) => (item.modify));
            } else {
                throw "reach unsupported element when collecting edited values";
            }
        });
        return {origin:origin , modify:modify};
    } // end of _diff_iter

    differ() {
        return this.state.saved.map(this._diff_iter.bind(this));
    }
    
    _serializer_reducer(key, value) {
        var skip_fields = ["refs", "_order_idx"];
        return skip_fields.indexOf(key) >= 0 ? undefined: value;
    }
    
    gather_unsaved_items(difference) {
        clear_array(this._uncommitted_items.added);
        clear_props_object(this._uncommitted_items.edited);
        difference.map((item, idx) => {
            // serialize the nested data for comparison, exclude non-serializable fields
            var serialized_origin = JSON.stringify(item.origin);
            var serialized_modify = JSON.stringify(item.modify, this._serializer_reducer);
            if(serialized_origin !== serialized_modify) {
                var _id = item.modify[this._app_item_id_label];
                if(_id) {
                    this._uncommitted_items.edited[_id] = item.modify;
                } else {
                    this._uncommitted_items.added.push(item.modify);
                }
            }
            return item;
        });
    }

    save(kwargs) {
        var skipped = true;
        var difference = this.differ();
        this.gather_unsaved_items(difference);
        let urlhost = kwargs.urlhost ;
        let urlpath = kwargs.urlpath ;
        let callbacks = kwargs.callbacks ;
        // append internal callback to the end of callback list for clearing
        //  uncommitted items when the API endpoint is successfully called
        var callbacks_copied = APIconsumer.copy_callbacks(callbacks, ['succeed']);
        callbacks_copied.succeed.push(this._commit_callback_succeed.bind(this));

        if(!urlhost) { urlhost = BaseUrl.API_HOST; }
        var url = urlhost + urlpath ;
        var req_opt = this._prepare_save_api_req();
        // Note POST and PUT request will be sent concurrently , for extra check
        // between PUT and POST requests, application developers should overwrite
        // this method
        let fetch_fields = null;
        if(kwargs.fetch_fields) {
            fetch_fields = kwargs.fetch_fields;
        } else {
            fetch_fields = [this._app_item_id_label];
        }
        if(req_opt.post.body) {
            // TODO: fetch new IDs of all nested data recursively and
            // write them to newly-created item of the array state
            let addlist = [...this._uncommitted_items.added];
            this.api_caller.start({base_url:url, req_opt:req_opt.post, callbacks:callbacks_copied,
                params:{fields:fetch_fields}, extra:{addlist: addlist}, });
            skipped = false;
        }
        if(req_opt.put.body) {
            let editlist = {...this._uncommitted_items.edited};
            this.api_caller.start({base_url:url, req_opt:req_opt.put, callbacks:callbacks_copied,
                 params:{fields:fetch_fields}, extra:{editlist: editlist}, });
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
        req_opt.post.body = APIconsumer.serialize(this._uncommitted_items.added,  valid_field_names, this._serializer_reducer);
        req_opt.put.body  = APIconsumer.serialize(this._uncommitted_items.edited, valid_field_names, this._serializer_reducer);
        return req_opt;
    } // end of _prepare_save_api_req
    

    _saved_item_data_copy(item, data) {
        // fetch new IDs of all nested data recursively and
        // write them to newly-created item of the array state
        if(data && data[this._app_item_id_label]) {
            item[this._app_item_id_label] = data[this._app_item_id_label];
        }
        this._valid_fields_name.map((fieldname) => {
            let elm = item.refs[fieldname].current;
            if((elm instanceof HTMLInputElement) || (elm instanceof HTMLSelectElement))
            {
                item[fieldname] = elm.value;
            } else if(elm instanceof BaseExtensibleForm){
                let nested_data = data ? data[fieldname] : undefined;
                if(nested_data) {
                    if(!nested_data.length) {
                        nested_data = [nested_data];
                    }
                    if(nested_data.length !== elm.state.saved.length) {
                        throw "[_saved_item_data_copy] length does not match, nested_data.length: "+ 
                            nested_data.length +", elm.state.saved.length:"+ elm.state.saved.length;
                    }
                }
                item[fieldname] = [];
                elm.state.saved.map((nested_saveditem, idx) => {
                    let nested_dataitem = nested_data ? nested_data[idx]: undefined;
                    elm._saved_item_data_copy(nested_saveditem, nested_dataitem);
                    let saveditem_puredata = {};
                    let copy_fields_name = [...elm._valid_fields_name];
                    if(nested_saveditem[elm._app_item_id_label]) {
                        copy_fields_name.push(elm._app_item_id_label);
                    }
                    copy_fields_name.map((nestfieldname) => {
                        saveditem_puredata[nestfieldname] = nested_saveditem[nestfieldname];
                    });
                    item[fieldname].push(saveditem_puredata);
                });
            } else {
                throw "[_saved_item_data_copy] unknown field type "+ elm;
            }
            return fieldname;
        });
    } // end of _saved_item_data_copy()

    _commit_callback_succeed(data, res, props) {
        var req_args = res.req_args;
        var req_mthd = req_args.req_opt.method ;
        if(req_mthd === 'POST') {
            this._uncommitted_items.added.map((val, idx) => {
                this._saved_item_data_copy(val.refs.saved_state_item, data[idx]);
                return val;
            });
            clear_array(this._uncommitted_items.added);
        } else if(req_mthd === 'PUT') {
            var edited_array = Object.entries(this._uncommitted_items.edited).map(kv => kv[1]);
            edited_array.map((val, idx) => {
                this._saved_item_data_copy(val.refs.saved_state_item);
                return val;
            });
            clear_props_object(this._uncommitted_items.edited);
        } else if(req_mthd === 'DELETE') {
            var varlist = req_args.extra.varlist;
            varlist.map((val, idx) => {
                this.remove_item(val, false);
                return val; 
            });
        } else if(req_mthd === 'PATCH') {
            let datalist = data.results ? data.results: data;
            datalist.map((val, idx) => {
                this.new_item(val, false);
            });
        }
        this.setState({saved: this.state.saved});
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
            if(kwargs.query_params) {
                query_params = {...query_params, ...kwargs.query_params}
            }
            let req_opt    = DEFAULT_API_REQUEST_OPTIONS.GET();
            let callbacks  = null;
            let bound_internal_callback = this._refresh_callback_succeed.bind(this);
            if(kwargs.callbacks) { // do partial copy, then add extra callback for internal use
                callbacks = APIconsumer.copy_callbacks(kwargs.callbacks, ['succeed']);
                // callbacks.succeed.push(bound_internal_callback);
                // always insert this internal function at first callback list,
                // always execute this internal function prior to all other caller-specified callbacks
                callbacks.succeed.splice(0, 0, bound_internal_callback);
            } else {
                callbacks = {succeed : [bound_internal_callback],};
            }
            if(kwargs.body) {
                // TRICKY , javascript API-consumer functions like Request() or fetch()
                // don't  allow client to send request with body data, which is
                // very inconvenient, (while HTTP spec doesn't forbid you to do so), these
                // functions silently cancel GET request with body data.
                // So I need to encode the body data then put it to URI query parameter
                // ,which has size limit ..... inconvenient again.
                //req_opt.body = kwargs.body;
            }
            let extra = kwargs.extra;
            let urlhost = kwargs.urlhost ;
            if(!urlhost) { urlhost = BaseUrl.API_HOST; }
            this.api_caller.start({base_url: urlhost + api_url, req_opt:req_opt,
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
        let urlhost = kwargs.urlhost ;
        if(!urlhost) { urlhost = BaseUrl.API_HOST; }
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
            var url = urlhost + urlpath ;
            var valid_field_names = kwargs.valid_field_names ? kwargs.valid_field_names: [this._app_item_id_label,];
            var req_opt = DEFAULT_API_REQUEST_OPTIONS.DELETE()
            req_opt.body = APIconsumer.serialize(varlist,  valid_field_names, this._serializer_reducer);
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
            let urlhost = kwargs.urlhost ;
            if(!urlhost) { urlhost = BaseUrl.API_HOST; }
            var url = urlhost + urlpath ;
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
        let _new_state_item = {refs:{} , _order_idx: this._unique_key_increment};
        // TODO: set up list of forbidden field names
        _new_state_item.refs.saved_state_item = _new_state_item;
        this._unique_key_increment += 1;
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
        saved.splice(0, 0, _new_state_item);
        if(update_state) {
            this.setState({saved: saved});
        }
        return _new_state_item;
    } // end of new_item()

    remove_item(val, update_state) {
        let idx = this.state.saved.indexOf(val);
        if(idx >= 0) {
            this.state.saved.splice(idx, 1);
            if(update_state) {
                this.setState((prev_state) => ({
                    saved : this.state.saved,
                }));
            }
        } else {
            console.log("the given item "+ val +" does not exist in component state");
        }
    } // end of remove_item()

    _single_item_render(val, idx) {
        throw "not implemented yet";
    }

    _single_item_render_wrapper(val, idx) {
        let result = this._single_item_render(val, idx);
        // TRICKY , always set up unique-key field in each object for the
        // array of objects in the component state.
        // DO NOT use index of each array item as the unique key, React.JS
        // doesn't seem to work well for unknown reason if you do so.
        return (<div key={val._order_idx}>{ result }</div>);
    }

    render() {
        return (
            <div className="row" id="dynamic_form_wrapper">
            { this.state.saved.map(this._single_item_render_wrapper.bind(this)) }
            </div>
        );
    }
}; // end of class BaseExtensibleForm


