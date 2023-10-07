use order::api::web::dto::{ContactReqDto, PhoneNumberReqDto};
use order::model::ContactModel;


#[test]
fn convert_dto_ok()
{
    let data = ContactReqDto {first_name:"Hussien".to_string(), last_name:"Flyar".to_string(),
        phones:vec![PhoneNumberReqDto {nation:9, number:"1802885".to_string()},
           PhoneNumberReqDto {nation:72, number:"00812116".to_string()} ],
        emails:vec!["ggla@hommy.idv".to_string(), "996icu@txcwok.cc".to_string()]
    };
    let result = ContactModel::try_from(data);
    assert!(result.is_ok());
    if let Ok(v) = result {
        assert_eq!(v.last_name.as_str(), "Flyar");
        assert_eq!(v.phones.len(), 2);
        assert_eq!(v.emails.len(), 2);
        assert_eq!(v.phones[0].nation, 9);
        assert_eq!(v.phones[0].number.as_str(), "1802885");
        assert_eq!(v.emails[1].as_str(), "996icu@txcwok.cc");
    }
}

#[test]
fn convert_dto_error ()
{
    let data = ContactReqDto {first_name:"Hussien".to_string(), last_name:"Flyar".to_string(),
        phones:vec![PhoneNumberReqDto {nation:9, number:"1802885".to_string()},
           PhoneNumberReqDto {nation:72, number:"008l2116".to_string()} ],
        emails:vec!["ininder@falung.org".to_string(), "heiz@billykane.io@yt".to_string(),
            "anu/ser@@i-am-here.bot".to_string()]
    };
    let result = ContactModel::try_from(data);
    assert!(result.is_err());
    if let Err(v) = result {
        assert!(v.emails.is_some());
        assert!(v.phones.is_some());
        if let Some(e) = v.emails.as_ref() {
            assert!(e[0].is_none());
            assert!(e[1].is_some());
            assert!(e[2].is_some());
        }
        if let Some(p) = v.phones.as_ref() {
            assert!(p[0].is_none());
            assert!(p[1].is_some());
        }
    }
}

