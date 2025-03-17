# E-commerce Backend Platform
## Overview
This project comprises several independent backend applications, each responsible for a specific business domain in B2C or B2B scenarios, from user management to payment processing.

This project is currently for learning purpose and not production-ready system.

|Application Name|Purpose|
|----------------|-------|
|[User Management](./services/user_management)|handles authentication, registration, and user profile maintenance|
|[Product](./services/product/v2)|manages the product catalog search, including custom attributes and taggings |
|[Storefront](./services/store)|manages storefronts profile, product pricing, other business-specific configurations |
|[Media Processing](./services/media)|responsible for storing, processing, and serving media assets (e.g., product images, videos)|
|[Order Processing](./services/order)|takes care of order creation, tracking, and return|
|[Payment](./services/payment)|integrates with third-party gateways to process payments and manage transaction states|

## Architecture and Design
- Service-Oriented Architecture, where each application operates independently.
- API-Driven Communication
  - frontend applications can access these backend applicaions through web API endpoints
  - communications between the backend applications can be achieved with asynchronous messaging through RPC API whenever needed.
- Hexagonal Architecture in Product-catalogue, Media-Processing, order-processing, and payment applications.
- Test-Driven Development, unit / integration tests cover all primary features of the applications

> Note: Currently, this project has not been deployed in any production environment. Further refinements may be needed based on real-world scaling and performance observations.

## Build, Test, and Development
- setup for [inter-service communication](./INTER_SERVICE.md)
- each application comes with its own build, test processes, and technology stack details, please refer to `README` of these application

## License
This project is licensed under [Your License Here]. See the [LICENSE](./LICENSE) file for details.

