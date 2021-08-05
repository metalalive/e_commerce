import React from 'react';

import {BaseUrl} from '../js/constants.js';
import {toggle_visual_elm_showup, _instant_search} from '../js/common/native.js';
import {CommonPresentableProductItemForm} from './CommonPresentableProductItemForm.js';
import {TagsAppliedToItem} from '../components/TagsAppliedToItem.js'
import {SaleableItemPicture} from '../components/SaleableItemPicture.js'
import {IngredientsAppliedToSaleableItem} from '../components/IngredientsAppliedToSaleableItem.js'


let api_base_url = {
    item_plural:   {host:BaseUrl.API_HOST , path:'/product/saleableitems'},
    item_singular: {host:BaseUrl.API_HOST , path:'/product/saleableitem/{0}'},
};

let refs = {form_items: React.createRef()};

function _new_empty_form(evt) {
    let form_ref = this.current;
    form_ref.new_item(undefined, true);
}

function _save_items(evt) {
    let form_ref = this.current;
    let kwargs = {urlpath:api_base_url.item_plural.path , urlhost:api_base_url.item_plural.host, 
        fetch_fields:['id', 'tags', 'attributes'], };
    form_ref.save(kwargs);
}

class SaleableItems extends CommonPresentableProductItemForm {
    constructor(props) {
        let _valid_fields_name = ['visible', 'price', 'tags', 'ingredients_applied', 'media_set']; 
        super(props, _valid_fields_name);
    }

    componentDidMount() {
        let attributes = [
            {id:71,  type:10,  value:"goat", extra_amount:0.5 },
            {id:75,  type:11,  value:2.711,  extra_amount:0.7 },
            {id:123, type:12,  value:-29,    extra_amount:1.3 },
        ];
        let tags = [
            {id:2 , tag_id:100,},
            {id:5 , tag_id:65,},
        ];
        let media_set = [
            {thumbnail:"blob:http://localhost:3000/32t43094.jpg", resource_id:"t934it3rjf"},
            {thumbnail:"blob:http://localhost:3000/093ur20u.jpg", resource_id:"g2h43gi3q4"},
            {thumbnail:"blob:http://localhost:3000/RI49g3gf.jpg", resource_id:"MQOR43t34t"},
            {thumbnail:"blob:http://localhost:3000/94jgij3m.jpg", resource_id:"bb9jrmrqjl"},
        ];
        let ingredients_applied = [
            {id: 20,  ingredient_id: 32,  unit:141,  quantity: 3.4},
            {id: 185, ingredient_id: 30,  unit:1,    quantity: 8},
            {id: 91,  ingredient_id: 4,   unit:199,  quantity: 12.7},
        ];
        let val = { name:'pee meat ball',  id:51, visible: true, price: 25.04, tags:tags,
            attributes:attributes, media_set:media_set, ingredients_applied:ingredients_applied };
        this.new_item(val, true);
    }

    _normalize_fn_price(value) {
        return parseFloat(value);
    }

    _normalize_fn_visible(value, referrer) {
        if(referrer instanceof HTMLInputElement) {
            value = referrer.checked;
        }
        return (value === "true" || value === true);
    }

    new_item(val, update_state) {
        var item = CommonPresentableProductItemForm.prototype.new_item.call(
                this, val, update_state);
        if(!item.name) {
            item.name = "<new saleable item>";
        }
        return item;
    }
    
    _single_item_visible_field_render(val, idx) {
        return (
            <label className="form-check">
                <input type="checkbox" className="form-check-input" name="example-text-input"
                    ref={val.refs.visible} defaultChecked={val.visible} />
                <span className="form-check-label"> Visible in front store </span>
            </label>
        );
    }
    
    _single_item_price_field_render(val, idx) {
        return (
            <label className="form-label">
                Base Price (excluding extra fee for chosen attributes)
                <input type="text" className="form-control" name="example-text-input"
                    ref={val.refs.price} defaultValue={val.price} />
            </label>
        );
    }
    _single_item_tags_field_render(val, idx) {
        return <label className="form-label">
                Tags applied to this item
                <TagsAppliedToItem  defaultValue={val.tags} ref={val.refs.tags} />
            </label> ;
    }

    _single_item_mediaset_field_render(val, idx) {
        return <div className="col-md-6 col-xl-12 ">
                <label className="form-label"> Pictures</label>
                <SaleableItemPicture defaultValue={val.media_set} ref={val.refs.media_set} />
            </div> ;
    }

    _single_item_ingredients_field_render(val, idx) {
        return <label className="form-label">
                Ingredients applied to this product item
                <IngredientsAppliedToSaleableItem  defaultValue={val.ingredients_applied}
                    ref={val.refs.ingredients_applied} />
            </label> ;
    }

    _single_item_menu_render(val, idx) {
        let name_field    = this._single_item_name_field_render(val, idx);
        let visible_field = this._single_item_visible_field_render(val, idx);
        let price_field   = this._single_item_price_field_render(val, idx);
        let tags_field    = this._single_item_tags_field_render(val, idx);
        let media_field   = this._single_item_mediaset_field_render(val, idx);
        let attributes_field  = this._single_item_attributes_field_render(val, idx, true);
        let ingredients_field = this._single_item_ingredients_field_render(val, idx);
        return (<>
                {visible_field} {name_field} {price_field} {tags_field}
                {media_field} {attributes_field} {ingredients_field}
            </>);
    } // end of _single_item_menu_render()
} // end of class SaleableItems


const SaleableItemsLayout = (props) => {
    let _sale_items = <SaleableItems ref={refs.form_items} />
    //let search_bound_obj = {search_api_fn: _instant_search_call_api};
    return (
        <>
          <div className="content">
          <div className="container-xl">
              <div className="row">
              <div className="col-xl-12">
                  <p>
                    all saleable items, attributes applied to them,
                    the media files to describe them, price estimate,
                    and price history of each item ...
                  </p>
                  <div className="card">
                    <div className="card-header">
                      <div className="d-flex">
                          <button className="btn btn-primary btn-pill ml-auto" onClick={ _new_empty_form.bind(refs.form_items) } >
                              new empty form
                          </button>
                          <button className="btn btn-primary btn-pill ml-auto" onClick={ _save_items.bind(refs.form_items) } >
                              Save
                          </button>
                      </div>
                    </div>
                    <div className="list list-row col-xl-8">
                        {_sale_items}
                    </div>
                  </div>
              </div>
              </div>
          </div>
          </div>
        </>
    );
};

export default SaleableItemsLayout;

