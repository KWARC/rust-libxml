#include "helper_functions.h"

/*
 * Helper functions for tree
 */

xmlNodePtr xmlNextSibling(const xmlNodePtr cur) {
    return cur->next;
}

xmlNodePtr xmlPrevSibling(const xmlNodePtr cur) {
    return cur->prev;
}

xmlNodePtr xmlGetFirstChild(const xmlNodePtr cur) {
    return cur->children;
}

xmlNodePtr xmlGetParent(const xmlNodePtr cur) {
    return cur->parent;
}

xmlDocPtr xmlGetDoc(const xmlNodePtr cur) {
    return cur->doc;
}

int xmlGetNodeType(const xmlNodePtr cur) {
    return cur->type;
}

const char * xmlNodeGetName(const xmlNodePtr cur) {
    return (char *) cur->name;
}

const char * xmlNodeGetContentPointer(const xmlNodePtr cur) {
    return (char *) cur->content;
}

xmlAttrPtr xmlGetFirstProperty(const xmlNodePtr cur) {
    return cur->properties;
}

xmlAttrPtr xmlNextPropertySibling(const xmlAttrPtr cur) {
    return cur->next;
}

const char * xmlAttrName(const xmlAttrPtr cur) {
    return (char *) cur->name;
}

const char * xmlNsPrefix(const xmlNsPtr ns) {
    return (char *) ns->prefix;
}

const char * xmlNsHref(const xmlNsPtr ns) {
    return (char *) ns->href;
}

xmlNsPtr xmlNextNsSibling(const xmlNsPtr ns) {
    return ns->next;
}

xmlNsPtr xmlNodeNs(const xmlNodePtr cur) {
    return cur->ns;
}

xmlNsPtr xmlNodeNsDeclarations(const xmlNodePtr cur) {
    return cur->nsDef;
}


void setIndentTreeOutput(const int indent) {
  xmlIndentTreeOutput = indent;
}
int getIndentTreeOutput() {
  return xmlIndentTreeOutput;
}

// Taken from Nokogiri (https://github.com/sparklemotion/nokogiri/blob/24bb843327306d2d71e4b2dc337c1e327cbf4516/ext/nokogiri/xml_document.c#L64)
void xmlNodeRecursivelyRemoveNs(xmlNodePtr node)
{
  xmlNodePtr child ;
  xmlAttrPtr property ;

  xmlSetNs(node, NULL);

  for (child = node->children ; child ; child = child->next)
    xmlNodeRecursivelyRemoveNs(child);

  if (((node->type == XML_ELEMENT_NODE) ||
       (node->type == XML_XINCLUDE_START) ||
       (node->type == XML_XINCLUDE_END)) &&
      node->nsDef) {
    xmlFreeNsList(node->nsDef);
    node->nsDef = NULL;
  }

  if (node->type == XML_ELEMENT_NODE && node->properties != NULL) {
    property = node->properties ;
    while (property != NULL) {
      if (property->ns) property->ns = NULL ;
      property = property->next ;
    }
  }
}

/*
 * helper functions for xpath
 */

int xmlXPathObjectNumberOfNodes(const xmlXPathObjectPtr val) {
    if (!val) return -1;
    if (!val->nodesetval) return -2;
    return val->nodesetval->nodeNr;
}

xmlNodePtr xmlXPathObjectGetNode(const xmlXPathObjectPtr val, size_t index) {
    return val->nodesetval->nodeTab[index];
}

void xmlFreeXPathObject(xmlXPathObjectPtr val) {
    if (val->nodesetval) xmlFree(val->nodesetval->nodeTab);
    xmlFree(val->nodesetval);
    xmlFree(val);
}

/*
 * helper functions for parser
 */
static int hacky_well_formed = 0;

int htmlWellFormed(htmlParserCtxtPtr ctxt) {
  if (((ctxt != NULL) && ctxt->wellFormed) || hacky_well_formed) {
    return 1;
  } else {
    return 0;
  }
}

// dummy function: no debug output at all
void _ignoreInvalidTagsErrorFunc(void * userData __attribute__ ((unused)), xmlErrorPtr error) {
  if ((error != NULL) && (error->code == XML_HTML_UNKNOWN_TAG)) { // do not record invalid, in fact (out of despair) claim we ARE well-formed, when a tag is invalid.
    hacky_well_formed = 1;
  }
  return;
}
void setWellFormednessHandler(htmlParserCtxtPtr ctxt) {
  hacky_well_formed = 0;
  xmlSetStructuredErrorFunc(ctxt, _ignoreInvalidTagsErrorFunc);
}
