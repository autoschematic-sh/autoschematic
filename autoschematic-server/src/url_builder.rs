#[allow(dead_code)]
#[derive(Debug)]
pub struct URLBuilder {
    protocol: String,
    host: String,
    port: u16,
    params: Vec<(String, String)>,
    routes: Vec<String>,
}

impl Default for URLBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl URLBuilder {
    /// Creates a new URLBuilder instance
    ///
    /// # Example
    ///
    /// ```
    /// use url_builder::URLBuilder;
    ///
    /// let mut ub = URLBuilder::new();
    /// ```
    pub fn new() -> URLBuilder {
        URLBuilder {
            protocol: String::new(),
            host: String::new(),
            port: 0,
            params: Vec::new(),
            routes: Vec::new(),
        }
    }

    /// Consumes the builder and returns a String, with the formatted
    /// url.
    ///
    /// # Example
    ///
    /// ```
    /// use url_builder::URLBuilder;
    ///
    /// let mut ub = URLBuilder::new();
    /// ub.set_protocol("http")
    ///     .set_host("localhost")
    ///     .set_port(8000)
    ///     .add_route("query")
    ///     .add_param("first", "1")
    ///     .add_param("second", "2")
    ///     .add_param("third", "3");
    ///
    /// let built_url = ub.build();
    /// ```
    pub fn build(self) -> String {
        let base = format!("{}://{}", self.protocol, self.host);

        let mut url_params = String::new();
        let mut routes = String::new();

        for route in self.routes {
            routes.push_str(format!("/{route}").as_str());
        }

        if !self.params.is_empty() {
            url_params.push('?');

            for (param, value) in self.params.iter() {
                url_params.push_str(format!("{param}={value}&").as_str());
            }

            // Remove the trailing `&`
            url_params.pop();
        }

        match self.port {
            0 => format!("{base}{routes}{url_params}"),
            _ => format!("{}:{}{}{}", base, self.port, routes, url_params),
        }
    }

    /// Adds a parameter to the URL.
    pub fn add_param(&mut self, param: &str, value: &str) -> &mut Self {
        self.params.push((param.to_string(), value.to_string()));

        self
    }

    /// Sets the protocol that the URL builder will use.
    pub fn set_protocol(&mut self, protocol: &str) -> &mut Self {
        self.protocol = protocol.to_string();

        self
    }

    /// Sets the protocol that the URL builder will use.
    pub fn set_host(&mut self, host: &str) -> &mut Self {
        self.host = host.to_string();

        self
    }

    /// Sets the port that the URL builder will use.
    pub fn set_port(&mut self, port: u16) -> &mut Self {
        self.port = port;

        self
    }

    /// Adds a route to the URL.
    pub fn add_route(&mut self, route: &str) -> &mut Self {
        self.routes.push(route.to_owned());

        self
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn protocol(&self) -> &str {
        &self.protocol
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_host() {
        let mut ub = URLBuilder::new();
        ub.set_host("localhost");
        assert_eq!("localhost", ub.host());
    }

    #[test]
    fn test_set_protocol() {
        let mut ub = URLBuilder::new();
        ub.set_protocol("https");
        assert_eq!("https", ub.protocol());
    }

    #[test]
    fn test_set_port() {
        let mut ub = URLBuilder::new();
        ub.set_port(8000);
        assert_eq!(8000, ub.port());
    }

    #[test]
    fn create_google_url() {
        let mut ub = URLBuilder::new();
        ub.set_protocol("http").set_host("www.google.com").set_port(80);
        let url = ub.build();
        assert_eq!("http://www.google.com:80", url);
    }

    #[test]
    fn create_url_without_port() {
        let mut ub = URLBuilder::new();
        ub.set_protocol("http").set_host("google.com");
        let url = ub.build();
        assert_eq!("http://google.com", url)
    }

    #[test]
    fn create_url_without_port_and_params() {
        let mut ub = URLBuilder::new();
        ub.set_protocol("http").set_host("google.com").add_param("gcookie", "0xcafe");
        let url = ub.build();
        assert_eq!("http://google.com?gcookie=0xcafe", url)
    }

    #[test]
    fn create_url_without_port_and_multiple_params() {
        let mut ub = URLBuilder::new();
        ub.set_protocol("http")
            .set_host("google.com")
            .add_param("gcookie", "0xcafe")
            .add_param("search", "rust")
            .add_param("locale", "en-gb");
        let url = ub.build();
        assert!(url.contains("gcookie=0xcafe"));
        assert!(url.contains("search=rust"));
        assert!(url.contains("locale=en-gb"));
    }

    #[test]
    fn create_url_with_routes() {
        let mut ub = URLBuilder::new();
        ub.set_protocol("http").set_host("google.com").add_route("mail");
        let url = ub.build();
        assert_eq!("http://google.com/mail", url)
    }

    #[test]
    fn create_url_with_params() {
        let mut ub = URLBuilder::new();
        ub.set_protocol("http")
            .set_host("localhost")
            .set_port(8000)
            .add_param("first", "1")
            .add_param("second", "2")
            .add_param("third", "3");

        let url = ub.build();
        assert!(url.contains("first=1"));
        assert!(url.contains("second=2"));
        assert!(url.contains("third=3"));
    }

    #[test]
    fn create_url_with_ports_routes_and_params() {
        let mut ub = URLBuilder::new();
        ub.set_protocol("http")
            .set_host("localhost")
            .set_port(8000)
            .add_route("query")
            .add_route("chains")
            .add_param("first", "1")
            .add_param("second", "2")
            .add_param("third", "3");

        let url = ub.build();
        assert!(url.contains("/query"));
        assert!(url.contains("/chains"));
        assert!(url.contains("/query/chains"));
        assert!(url.contains("first=1"));
        assert!(url.contains("second=2"));
        assert!(url.contains("third=3"));
    }
}
